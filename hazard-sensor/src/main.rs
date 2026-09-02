#![no_std]
#![no_main]

use {crate::sensors::wind_sensor::WindSensor, core::error::Error, defmt_rtt as _, panic_probe as _};
use crate::network::{BANDWIDTH, CODING_RATE, FREQ_HZ, MAX_PACKET_LEN, SPREADING_FACTOR, TX_POWER_DBM, Packet, NodeIdHash, PayloadType, RouteType};
// use {crate::newtork::...} Other functions from network will be needed

mod sensors;
mod network;

// use cortex_m::delay::Delay;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel; // Need to resolve channel import overlap
use embassy_nrf::gpio::{Level, Output, OutputDrive, Input, Pull};
use sensors::wind_sensor;
use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_nrf::*;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use static_cell::ConstStaticCell;

use lora_phy::sx126x::{self, Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::{DelayNs, LoRa, iv, RxMode}; 
use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor, ModulationParams, PacketParams};

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
});

// i2c tx buffer
static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);


// Initialise recieved packet buffer - currently of size 4 
// static MESHCORE_RX_BUFF: Channel<NoopRawMutex, heapless::Vec<u8, { MAX_PACKET_LEN+1 }>, 4> = Channel::new();

// Initialise tx packet buffer
static MESHCORE_TX_BUFF: Channel<CriticalSectionRawMutex, heapless::Vec<u8, { MAX_PACKET_LEN+1 }>, 4> = Channel::new();
// NoopRawMutex has sync error from use in main (async) and radio_task (async)
// static MESHCORE_TX_BUFF: Channel<NoopRawMutex, heapless::Vec<u8, { MAX_PACKET_LEN+1 }>, 4> = Channel::new();
// MESHCORE_TX_BUFF: Defines packet transmission buffer where each packet is 256 (u8) 
// bytes, and can store 4 packets at a time, can be increased. 
// MAX_PACKET_LEN+1 = 256 (Redundancy) 
// BUFF_SIZE (4), was chosen as a power of 2 for memory efficiency
//CriticalSelectionRawMutex maybe more appropriate depedning on async/
// interrupt implementation, as far as I can tell embassy_executor will 
// only access it from a single task it spawns at a time.

pub struct NRF52840 {
    i2c: twim::Twim<'static>,
}

impl NRF52840 {
    pub fn new(
        twispi0: Peri<'static, peripherals::TWISPI0>, 
        sda: Peri<'static, peripherals::P0_13>, 
        scl: Peri<'static, peripherals::P0_14>,
    ) -> Self { // Changed the input to take pins since p is singleton. p must not be created and passed from main 
        // let p = embassy_nrf::init(Default::default());
        let i2c_config = twim::Config::default(); 

        // Initialize the TWIM driver
        let mut i2c = twim::Twim::new(twispi0, Irqs, sda, scl, i2c_config, TX_BUFF.take()); 

        NRF52840 {
            i2c,
        }
    }
}

pub struct SX1262 {
    lora_radio: LoRa<Sx126x<ExclusiveDevice<spim::Spim<'static>, gpio::Output<'static>, embassy_time::Delay>,
            iv::GenericSx126xInterfaceVariant<gpio::Output<'static>, gpio::Input<'static>>,
            Sx1262,>,embassy_time::Delay,>,
    mod_params: ModulationParams,
    rx_pkt_params: PacketParams, 
    tx_pkt_params: PacketParams, 
}

impl SX1262 {
    pub async fn new(
        // SX1262 pins
        reset: Peri<'static, peripherals::P1_06>, 
        busy: Peri<'static, peripherals::P1_14>, 
        dio1: Peri<'static, peripherals::P1_15>, 
        rf_tx_en: Peri<'static, peripherals::P1_07>,
        rf_rx_en: Peri<'static, peripherals::P1_05>,
        // spi bus pins
        spi3: Peri<'static, peripherals::SPI3>,
        sck: Peri<'static, peripherals::P1_11>,
        miso: Peri<'static, peripherals::P1_13>,
        mosi: Peri<'static, peripherals::P1_12>,
        nss: Peri<'static, peripherals::P1_10>,
    ) -> Self {
        // Convert pins to outputs/inputs
        let reset_o = Output::new(reset, Level::High, OutputDrive::Standard); //NRESET P1.06
        let busy_i = Input::new(busy, Pull::None); //BUSY P1.14
        let dio1_i = Input::new(dio1, Pull::Down); //DIO1 P1.15 
        let rf_tx_en_o = Output::new(rf_tx_en, Level::High, OutputDrive::Standard);
        let rf_rx_en_o = Output::new(rf_rx_en, Level::High, OutputDrive::Standard); //ANT_SW P1.05
        // nss_pin as outpuit
        let nss_o = Output::new(nss, Level::High, OutputDrive::Standard);

        // Setup spi config
        let spi_config = spim::Config::default(); 
        // For non-default spi config change above to mut and use key fields below:
        // spi_config.frequency
        // spi_config.mode 

        // Setup Sx1262 config
        let sx1262_config = sx126x::Config {
            chip: Sx1262,
            tcxo_ctrl: Some(sx126x::TcxoCtrlVoltage::Ctrl1V8),
            use_dcdc: true, //use_dio2_as_rfswitch - deprecated format for lora-phy
            rx_boost: true, //rx boost useful?
        };
        
        // Initialise InterfaceVariant lora-phy object
        // let iv = lora_phy::iv::GenericSx126xInterfaceVariant::new(reset, dio1, busy, Some(rf_switch_rx), Some(rf_switch_tx)).unwrap();
        let iv = match lora_phy::iv::GenericSx126xInterfaceVariant::new(reset_o, dio1_i, busy_i, Some(rf_rx_en_o), Some(rf_tx_en_o)) {
            Ok(iv) => iv,
            Err(e) => {
                defmt::error!("failed to build SX1262 interface variant: {:?}", defmt::Debug2Format(&e));
                panic!("interface variant intialisation failed");
            }
        };

        // Initialise the SPI driver between the nRF52 and the Sx1262
        let spi = spim::Spim::new(spi3, Irqs, sck, miso, mosi, spi_config);

        // Initialise spi_device to pass to sx1262 constructor
        let spi_device = ExclusiveDevice::new(spi, nss_o, Delay)
            .expect("failed to build spi_device");

        // Initialise Sx1262
        let sx1262 = Sx126x::new(spi_device, iv, sx1262_config);

        // Initialise LoRa radio for MeshCore LoRa communications
        let mut lora_radio = LoRa::new(sx1262, false, Delay)
            .await //maybe need to extract from result here? 
            .expect("failed to initalise SX1262 - lora_radio object"); 

            // These functions may be better suited to be used in radio_task or main 
        // Initialise modulation parameters attached to lora_radio field of SX1262 object 
        let mod_params: ModulationParams = lora_radio.create_modulation_params(SPREADING_FACTOR, BANDWIDTH, CODING_RATE, FREQ_HZ)
            .expect("failed to create modulation params in SX1262 constructor.");

        // Initialise recieved packet parameters attached to lora_radio field of SX1262 object - these parameters are 
        // assumed as constants since the user that sends a query should have the parameters.
        let rx_pkt_params: PacketParams = lora_radio.create_rx_packet_params(8, false, 255, true, false, &mod_params)
            .expect("failed to create rx packet params in SX1262 constructor.");

        // Initialise transmitted packet parameters attached to lora_radio field of SX1262 object.
        // These parameters are fixed as stated in LHN documentation.
        let tx_pkt_params: PacketParams = lora_radio.create_tx_packet_params(8, false, true, false, &mod_params)
            .expect("failed to create tx packet params in SX1262 constructor."); 

        // Construct SX1262 object
        SX1262 { 
            lora_radio,
            mod_params,
            rx_pkt_params,
            tx_pkt_params,
        } 
    }
}

// main
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialisation of peripherals struct p 
    let p = embassy_nrf::init(Default::default());

    // nRF52 pin definitions to pass to constructor
    let twispi0 = p.TWISPI0;
    let sda = p.P0_13; //sda: peripherals::P0_13
    let scl = p.P0_14; //scl: peripherals::P0_14

    // SX1262 pin definitions to pass to constructor
    let reset = p.P1_06;
    let busy = p.P1_14; 
    let dio1 = p.P1_15; 
    let rf_tx_en = p.P1_07;
    let rf_rx_en = p.P1_05;
    // spi pins for SX1262
    let spi3 = p.SPI3;
    let sck = p.P1_11;
    let miso = p.P1_13;
    let mosi = p.P1_12;
    let nss = p.P1_10;

    // Initalisation of custom mcu and transciever objects
    let mcu = NRF52840::new(twispi0,sda,scl);
    let radio = SX1262::new(reset, busy, dio1, 
        rf_tx_en, rf_rx_en, spi3, sck, miso, mosi, nss).await;

    // let ws = WindSensor::new(mcu.i2c);

    // Start async radio task - buffers global therefore dont need to be passed 
    _spawner.spawn(radio_task(radio)).unwrap();
    
    // TESTING LOOP 
    // Testing data
    let wss_data: i16 = 1;
    let wds_data: u16 = 2;
    let aqs_data: (i32, u32, u32, i32) = (3, 3, 3, 3);
    let sms_data: i16 = 4;
    let tbs_data: f32 = 5.0;

    // Loop simulated data packet as origin 
    loop {
        Timer::after(Duration::from_secs(10)).await;
        // Prepare encoded payload
        let json = match Packet::encode_payload(&wss_data, &wds_data, &aqs_data, &sms_data, &tbs_data) {
            Ok(json) => json,
            Err(e) => {
                warn!("failed to encode sensor payload in main testing loop: {:?}", defmt::Debug2Format(&e));
                continue;
            }
        };
        // Prepare original outgoing packet
        let test_packet = match Packet::originate(RouteType::Flood, PayloadType::RawCustom, json.as_bytes()) {
            Ok(pkt) => pkt,
            Err(e) => {
                warn!("failed to originate packet: {:?}", defmt::Debug2Format(&e));
                continue;
            }
        };

        // Encode packet to buf - CHANGE THIS WITH GLOBAL BUF
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = match test_packet.encode(&mut buf) {
            Ok(len) => len,
            Err(e) => {
                warn!("failed to encode packet: {:?}", defmt::Debug2Format(&e));
                continue;
            }
        };

        // Extend packet to full frame
        let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
        if frame.extend_from_slice(&buf[..len]).is_err() {
            warn!("encoded frame too large for tx queue slot");
            continue;
        }

        // Try to send the formed frame
        if MESHCORE_TX_BUFF.try_send(frame).is_err() {
            warn!("tx queue full, dropping test packet");
        }
    }
}

#[embassy_executor::task]
async fn radio_task(
    mut radio: SX1262,
    // Buffers no longner global and may need to be passed in 
        // mut meshcore_tx_buff: [u8; 255],
        // mut meshcore_rx_buff: [[u8; 255]; 3],
){

    // Initialise rx pkt parameters - non-default 
    // if received_pkt.PacketParams != radio.rx_pkt_params {
    // let rx_pkt_params_non_std: PacketParams = radio.lora_radio.create_rx_pkt_params(...).expect();
    // }
    

    // Initialise recieved packet buffer locally - channel format deprecated 
    let mut meshcore_rx_buf = [0u8; MAX_PACKET_LEN];

    loop{
        while let Ok(frame) = MESHCORE_TX_BUFF.try_receive() {
            send_frame(&mut radio, &frame).await;
        }

        radio.lora_radio.prepare_for_rx(RxMode::Continuous, &radio.mod_params, &radio.rx_pkt_params)
            .await
            .expect("Prepare for rx (continuous mode) failed");

        match radio.lora_radio.rx(&radio.rx_pkt_params, &mut meshcore_rx_buf).await {
            Ok((len,status)) => {
                let data = &meshcore_rx_buf[..len as usize];
                info!("RX {} bytes, rssi={} snr={}", len, status.rssi, status.snr);
                handle_received(data);
            }
            Err(e) => {
                warn!("radio rx error {:?}", defmt::Debug2Format(&e));
            }
        }
    }
}

/// Send a frame - MeshCore packet inside LoRa envelope 
async fn send_frame(
    radio: &mut SX1262,
    frame: &[u8],
){
    // Ready the radio for transmission with a frame
    if let Err(e) = radio.lora_radio.prepare_for_tx(&radio.mod_params, &mut radio.tx_pkt_params, TX_POWER_DBM, frame)
    .await {
        warn!("prepare_for_tx failed in send_frame: {:?}", defmt::Debug2Format(&e));
        return;
    }
    // Attempt transmission
    if let Err(e) = radio.lora_radio.tx().await {
        warn!("tx failed in send_frame: {:?}", defmt::Debug2Format(&e));
    } else {
        info!("TX successful, {} bytes sent", frame.len())
    }
}

/// Process incoming packet data
pub fn handle_received(raw_packet_data: &[u8]) { // , ws: &mut WindSensor
    match Packet::decode(raw_packet_data) {
        Ok(pkt) => {
            // Log received packet info
            info!(
                "MeshCore packet: route={:?} type={:?} hops={} payload_len={}",
                defmt::Debug2Format(&pkt.route_type),
                defmt::Debug2Format(&pkt.payload_type),
                pkt.path.len(),
                pkt.payload.len(),
            );
            // Determine action based on rx payload type 
            match pkt.payload_type {
                PayloadType::Request => {
                        info!("Recieved Request packet; packet ignored.");
                        // Possible extension to move away from RawCustom reliance.
                        // Extension may be better suited to using GroupText for 
                        // both requests and responses
                }  
                PayloadType::Response => {
                    info!("Recieved Response packet; packet ignored.");
                }
                PayloadType::TextMessage => {
                    info!("Recieved TextMessage packet; packet ignored.");
                    // Feed into decryption func if extended to direct message 
                }
                PayloadType::Ack => {
                    info!("Recieved Ack packet; packet ignored.");
                }
                PayloadType::GroupText => {
                    info!("Recieved GroupText packet; packet ignored.");
                    // Feed into decryption func then process
                    // todo!()
                }
                PayloadType::GroupData => {
                    info!("Recieved GroupData packet; packet ignored.");
                }
                PayloadType::AnonRequest => {
                    info!("Recieved AnonRequest packet; packet ignored.");
                }
                PayloadType::Path => {
                    info!("Recieved Path packet; packet ignored.");
                }
                PayloadType::Trace => {
                    info!("Recieved Trace packet; packet ignored.");
                }
                PayloadType::Multipart => {
                    info!("Recieved Multipart packet; packet ignored.");
                }
                PayloadType::Control => {
                    info!("Recieved Control packet; packet ignored.");
                    // Possibly used with MQTT MeshCore extension for node health checks 
                }
                // Currently the key packet type, used to handle requests, responses, and timed broadcasts
                PayloadType::RawCustom => {
                    info!("Recieved RawCustom packet; packet ignored.");
                    // Look at first byte of received packet payload
                    match pkt.payload.split_first() {
                        // Match against designated request tag byte
                        Some((&network::RAW_CUSTOM_REQUEST_TAG, _rest)) => {
                            info!("received request for all sensor data — reading live");

                            // DUMMY DATA FOR RESPONSE - ONLY FOR TESTING
                            let wss_data = 1;
                            let wds_data: u16 = 2;
                            let aqs_data: (i32, u32, u32, i32) = (3, 3, 3, 3);
                            let sms_data: i16 = 4;
                            let tbs_data: f32 = 5.0;

                            // Attempt encoding payload using sensor data
                            match Packet::encode_payload(&wss_data, &wds_data, &aqs_data, &sms_data, &tbs_data) {
                                // ONLY FOR RAW CUSTOM RESPONSE - Prepend JSON formatted payload with designated response byte
                                Ok(json) => match network::build_sensor_data_frame(&json) {
                                    Ok(response_payload) => {
                                        // Copy received packet path then reverse it to obtain response path
                                        let mut return_path = pkt.path.clone();
                                        return_path.reverse();

                                        // Fully form MeshCore (raw custom) response packet 
                                        let response = Packet {
                                            payload_version: 0,
                                            route_type: RouteType::Direct,
                                            payload_type: PayloadType::RawCustom,
                                            transport_code: None,
                                            path: return_path,
                                            payload: &response_payload,
                                        };

                                        // Encode MeshCore packet into bytes for LoRa transmission
                                        let mut buf = [0u8; MAX_PACKET_LEN];
                                        match response.encode(&mut buf) {
                                            Ok(len) => {
                                                let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
                                                if frame.extend_from_slice(&buf[..len]).is_ok() {
                                                    if MESHCORE_TX_BUFF.try_send(frame).is_err() {
                                                        warn!("tx queue full, dropping response");
                                                    }
                                                }
                                            }
                                            Err(e) => warn!("failed to encode response packet: {:?}", defmt::Debug2Format(&e)),
                                        }
                                    }
                                    Err(e) => warn!("failed to build response frame: {:?}", defmt::Debug2Format(&e)),
                                },
                                Err(e) => warn!("failed to encode sensor payload: {:?}", defmt::Debug2Format(&e)),
                            }
                        }
                        // CODE FOR HANDLING RECIEVED RAW_CUSTOM DATA PACKET - ONLY FOR TESTING
                        Some((&network::RAW_CUSTOM_RESPONSE_TAG, rest)) => {
                            if let Ok(json_str) = core::str::from_utf8(rest) {
                                info!("received sensor JSON: {}", json_str);
                            } else {
                                warn!("sensor-data RawCustom payload was not valid UTF-8");
                            }
                        }
                        Some((other, _)) => warn!("RawCustom payload with unknown tag: {}", other),
                        None => warn!("RawCustom payload was empty"),
                    }
                }
                other => {
                    info!("unhandled payload type {:?}", defmt::Debug2Format(&other));
                }
            }
        }
        Err(e) => warn!("failed to parse packet in handle_received: {:?}", defmt::Debug2Format(&e)),
    }
}
