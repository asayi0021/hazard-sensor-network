#![no_std]
#![no_main]

use {crate::sensors::wind_sensor::WindSensor, core::error::Error, defmt_rtt as _, panic_probe as _};
use crate::network::{BANDWIDTH, CODING_RATE, FREQ_HZ, SPREADING_FACTOR};
// use {crate::newtork::...} Other functions from network will be needed

mod sensors;
mod network;

// use cortex_m::delay::Delay;
use embassy_nrf::ppi::Channel;
use embassy_nrf::gpio::{Level, Output, OutputDrive, Input, Pull};
use sensors::wind_sensor;
use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::*;
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use static_cell::ConstStaticCell;
// Used cargo add for meshcore_rs, but not using it, need to do cargo remove later

use lora_phy::sx126x::{self, Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::{DelayNs, LoRa, iv}; //RxMode - depricated in new lora-phy
use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor, ModulationParams, PacketParams};

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
});

// i2c tx buffer
static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

/// MESHCORE_TX_BUFF: Defines packet transmission buffer where each packet is 256 (u8) 
/// bytes, and can store 4 packets at a time, can be increased. 
/// MAX_PACKET_LEN+1 = 256 (Redundancy) 
/// BUFF_SIZE (4), was chosen as a power of 2 for memory efficiency
// static MESHCORE_TX_BUFF: Channel<NoopRawMutex, heapless::Vec<u8, 256>, 4> = Channel::new();
//CriticalSelectionRawMutex maybe more appropriate depedning on async/
// interrupt implementation, as far as I can tell embassy_executor will 
// only access it from a single task it spawns at a time.

// Initilise recieved packet buffer - currently of size 1 
// static MESHCORE_RX_BUFF = [0u8: 255];

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
    // Pulled into final field now, makes below fields both redundant and out of scope
    // spi: spim::Spim<'static>,
    // sx1262: sx126x::Sx1262<'static>,
    lora_radio: LoRa<Sx126x<ExclusiveDevice<spim::Spim<'static>, gpio::Output<'static>, embassy_time::Delay>,
            iv::GenericSx126xInterfaceVariant<gpio::Output<'static>, gpio::Input<'static>>,
            Sx1262,>,embassy_time::Delay,>  
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
        let mod_params: ModulationParams = lora_radio.create_modulation_params(SPREADING_FACTOR, BANDWIDTH, CODING_RATE, FREQ_HZ)
            .expect("failed to create modulation params");
        // let rx_pkt_params: PacketParams = lora.create_rx_packet_params(network::SPREADING_FACTOR, ... );
        let pkt_params: PacketParams = lora_radio.create_rx_packet_params(8, false, 255, true, false, &mod_params)
            .expect("failed to create rx packet params");

        // Construct SX1262 object
        SX1262 { 
            lora_radio
            // modulation_pararms_tx
            // mod_pkt_params_rx 
         } 
    }
}

// 
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




    info!("Hello, world!");

    _spawner.spawn(radio_task(radio)).unwrap();
}

#[embassy_executor::task]
async fn radio_task(
    mut radio: SX1262
){
    // Initialise modulation parameters - POSSIBLY MOVE THIS TO MAIN
    // let tx_mod_params: ModulationParams = radio.lora_radio
    // // = sx1262.lora_radio.create_modulation_params(SPREADING_FACTOR, BANDWIDTH, CODING_RATE, FREQ_HZ);
    // let rx_pkt_mod_params: PacketParams = radio.lora_radio.create_rx_pkt_params().expect();

    // Initialise recieved packet buffer
    // let mut TX

    // loop{}
}
