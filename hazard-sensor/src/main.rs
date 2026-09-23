#![no_std]
#![no_main]

// Module declaration
mod sensors;
mod network;

// Imports from other modules
use crate::network::{frame_handler, send_frame, poll_all, BANDWIDTH, CODING_RATE, FREQ_HZ, MAX_PACKET_LEN, SPREADING_FACTOR, TX_POWER_DBM};
use crate::sensors::{watchdog::{WatchdogTimer, WatchdogWindow},gas_sensor::GasSensor,adc_sensors::AdcSensors,tipping_bucket::RainfallSensor}; //

// Embassy imports
use embassy_futures::select::{select3, Either3};
use embassy_sync::{mutex::Mutex, blocking_mutex::raw::{NoopRawMutex, CriticalSectionRawMutex}, channel::Channel}; //{raw::NoopRawMutex, Mutex}
use embassy_nrf::*;
use embassy_nrf::{saadc::{ChannelConfig, Saadc},uarte::Uarte,gpio::{Level, Output, OutputDrive, Input, Pull}};
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;

// Other imports
use static_cell::{ConstStaticCell, StaticCell};
use core::sync::atomic::{AtomicI32, AtomicBool};
use lora_phy::{LoRa, iv, RxMode, sx126x::{self, Sx1262, Sx126x, TcxoCtrlVoltage}, mod_params::{ModulationParams, PacketParams}};
use {defmt_rtt as _, panic_probe as _};
use defmt::{info, warn, error};

// Binding interupts to different buses.
bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    TWISPI1 => twim::InterruptHandler<peripherals::TWISPI1>;
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
    UARTE1  => uarte::InterruptHandler<peripherals::UARTE1>;
    SAADC => saadc::InterruptHandler;
});

/// Transmission buffer for I2C Bus 1
static TX_BUFF1: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);
/// Transmission buffer for I2C Bus 2
static TX_BUFF2: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

/// I2C shared bus for RainfallSensor (tbs), GasSensor (aqs)
static I2C_BUS: StaticCell<Mutex<NoopRawMutex, twim::Twim>> = StaticCell::new();

/// Initialise tx packet buffer
pub static MESHCORE_TX_BUFF: Channel<CriticalSectionRawMutex, heapless::Vec<u8, { MAX_PACKET_LEN+1 }>, 4> = Channel::new();

/// Local clock, expressed as an offset applied to time-since-boot.
///   current_timestamp = seconds_since_boot + CLOCK_OFFSET
/// Starts at approx 12pm 15/09/26, meaning "current_timestamp" is just
/// that date plus the seconds-since-boot until synchronized — which
/// already guarantees every call returns a distinct, increasing value but may
/// fall outside the acceptable time window.
pub static CLOCK_OFFSET: AtomicI32 = AtomicI32::new(1789439994);
pub static CLOCK_SYNCED: AtomicBool = AtomicBool::new(false);

/// Gas sensor I2C slave address
const GAS_SENSOR_ADDR: u8 = 0x77;

/// Rainfall sensor I2C slave address
const RAINFALL_SENSOR_ADDR: u8 = 0x1D;

/// Shortened type for GasSensor and RainfallSensor objects
pub type I2cShared = I2cDevice<'static, NoopRawMutex, twim::Twim<'static>>;

/// nRF52 custom object for storing initialised buses for different peripheral protocols.
pub struct NRF52840 {
    i2c1: twim::Twim<'static>,
    i2c2: twim::Twim<'static>,
    saadc: Saadc<'static, 2>,
}

impl NRF52840 {
    pub fn new(
        twispi0: Peri<'static, peripherals::TWISPI0>,
        twispi1: Peri<'static, peripherals::TWISPI1>,
        sda1: Peri<'static, peripherals::P0_13>,
        scl1: Peri<'static, peripherals::P0_14>,
        sda2: Peri<'static, peripherals::P0_15>,
        scl2: Peri<'static, peripherals::P0_16>,
        saadc: Peri<'static, peripherals::SAADC>,
        adc_channel0: Peri<'static, peripherals::P0_31>,
        adc_channel1: Peri<'static, peripherals::P0_03>,
        // adc_channel2: Peri<'static, peripherals::P0_03>,
    ) -> Self {
        // Initialise config for each bus
        let i2c_config1 = twim::Config::default();
        let i2c_config2 = twim::Config::default();
        let adc_config = saadc::Config::default();

        // Initialise saadc channels
        let channel0 = ChannelConfig::single_ended(adc_channel0);
        let channel1 = ChannelConfig::single_ended(adc_channel1);
        // let channel2 = ChannelConfig::single_ended(adc_channel2);

        // Initialize the i2c drivers
        let i2c1 = twim::Twim::new(twispi0, Irqs, sda1, scl1, i2c_config1, TX_BUFF1.take());
        let i2c2 = twim::Twim::new(twispi1, Irqs, sda2, scl2, i2c_config2, TX_BUFF2.take());

        // Intitialise saadc
        let saadc = Saadc::new(saadc, Irqs, adc_config, [channel0, channel1]);
        // let saadc = Saadc::new(saadc, Irqs, adc_config, [channel0, channel1, channel2]);

        NRF52840 {
            i2c1,
            i2c2,
            saadc
        }
    }
}

/// Sx1262 Object for initalising the transceiver and it's assigned parameters.
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
        reset: Peri<'static, peripherals::P1_06>,    // In RAK4630 datasheet, the DIO2
        busy: Peri<'static, peripherals::P1_14>,     // is used to control antenna switch,
        dio1: Peri<'static, peripherals::P1_15>,     // and that the GPIO 1.07 pin should
        rf_tx_en: Peri<'static, peripherals::P1_07>, // and that the GPIO 1.07 pin should
        rf_rx_en: Peri<'static, peripherals::P1_05>, // not be initialised.
        // spi bus pins
        spi3: Peri<'static, peripherals::SPI3>,
        sck: Peri<'static, peripherals::P1_11>,
        miso: Peri<'static, peripherals::P1_13>,
        mosi: Peri<'static, peripherals::P1_12>,
        nss: Peri<'static, peripherals::P1_10>,
    ) -> Self {
        // Convert pins to outputs/inputs
        let reset_o = Output::new(reset, Level::High, OutputDrive::Standard);        //NRESET P1.06
        let busy_i = Input::new(busy, Pull::None);                                                          //BUSY P1.14
        let dio1_i = Input::new(dio1, Pull::Down);                                                          //DIO1 P1.15
        let rf_tx_en_o = Output::new(rf_tx_en, Level::High, OutputDrive::Standard);
        let rf_rx_en_o = Output::new(rf_rx_en, Level::High, OutputDrive::Standard);  //ANT_SW P1.05
        // nss_pin as output
        let nss_o = Output::new(nss, Level::High, OutputDrive::Standard);

        // Setup Sx1262 config
        let sx1262_config = sx126x::Config {
            chip: Sx1262,
            tcxo_ctrl: Some(sx126x::TcxoCtrlVoltage::Ctrl1V8),
            use_dcdc: true,
            rx_boost: true,
        };

        // Initialise InterfaceVariant lora-phy object
        let iv = match lora_phy::iv::GenericSx126xInterfaceVariant::new(reset_o, dio1_i, busy_i, Some(rf_rx_en_o), Some(rf_tx_en_o)) {
            Ok(iv) => iv,
            Err(e) => {
                defmt::error!("failed to build SX1262 interface variant: {:?}", defmt::Debug2Format(&e));
                panic!("interface variant intialisation failed");
            }
        };

        // Setup spi config for Sx1262
        let spi_config = spim::Config::default();

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

//======================================================================================================================
//----Main task---------------------------------------------------------------------------------------------------------
//======================================================================================================================

/// Main - initialises transceiver and peripherals, then spawns the radio_task.
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Main start");

    // Initialisation of peripherals struct p
    let p = embassy_nrf::init(Default::default());

    // gpio pins for MIKROE-4416 (watchdog)
    let io1 = Output::new(p.P0_17, Level::Low, OutputDrive::Standard); //wdi
    let io2: Output<'_> = Output::new(p.P1_02, Level::High, OutputDrive::Standard); //s0
    // NOTE: io2 is stated as a controlling pin for the 3V3_S supply voltage rail, it seems that this rail is seperate
    // from the 3V3 VDD pin on the J-headers and only matters for the sensor slots but this may be an error point.

    let mut watchdog = WatchdogTimer::new(io1, io2).unwrap();
    watchdog.set_window(WatchdogWindow::Disabled).unwrap();
    info!("Watchdog disabled");
    info!("RESESTABLISH RST LINK");
    Timer::after_secs(5).await;
    info!("Test 1");
    // Configure watchdog
    watchdog.set_window(WatchdogWindow::Ratio1To8).unwrap();
    info!("Set the watchdog window");
    // Wait out t_WD-setup with margin before WDI is recognized
    Timer::after_micros(500).await;
    info!("Waiting for watchdog setup time and for window");
    // Send the mandatory first pulse, well inside tWDU(min) = 92.7 ms
    watchdog.send_pulse().await.unwrap();
    info!("Watchdog WDI pulse sent");
    watchdog.set_window(WatchdogWindow::Disabled).unwrap();
    info!("Watchdog disabled");

    // nRF52 pin definitions to pass to constructors
    // i2c pins
    let twispi0 = p.TWISPI0;
    let twispi1 = p.TWISPI1;
    let sda1 = p.P0_13; //sda: peripherals::P0_13
    let scl1 = p.P0_14; //scl: peripherals::P0_14
    let sda2 = p.P0_15;
    let scl2 = p.P0_16;
    // adc pins
    let saadc = p.SAADC;
    let adc_channel0 = p.P0_31;
    let adc_channel1 = p.P0_03;
    // let adc_channel2 = p.P0_03; pin is AIN3, need to map to RAK4630 pin

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

    // Initialisation of RAK4630 objects
    let mcu = NRF52840::new(twispi0,twispi1,sda1,scl1,sda2,scl2,saadc,adc_channel0,adc_channel1);
    // let mcu = NRF52840::new(twispi0,twispi1,sda1,scl1,sda2,scl2,saadc,adc_channel0,adc_channel1,adc_channel2);
    let radio = SX1262::new(reset, busy, dio1, rf_tx_en, rf_rx_en, spi3, sck, miso, mosi, nss).await;

    // Initialisation of shared I2C bus
    let i2c_bus = Mutex::new(mcu.i2c1);
    let i2c_bus = I2C_BUS.init(i2c_bus);
    // Create seperate objects to pass to constructors for shared I2C bus
    let gas_i2c = I2cDevice::new(i2c_bus);
    let rain_i2c = I2cDevice::new(i2c_bus);

    // ADC sensors init - soil moisture (sms) and wind speed (wss)
    let adc = AdcSensors::new(mcu.saadc);

    // Gas sensor initialisation
    let mut aqs = match GasSensor::new(gas_i2c, GAS_SENSOR_ADDR).await { // currently on same i2c bus as tbs
        Ok(sensor) => {
            info!("GAS SENSOR initialised.");
            sensor
        },
        Err(e) => panic!("Could not intialise GAS SENSOR: {:?}", e),
    };
    match aqs.init_config().await {
        Ok(_) => info!("GAS SENSOR configuration success."),
        Err(err) => panic!("Failed to configure GAS SENSOR: {:?}", err),
    };

    // Tipping bucket (rainfall) sensor initialisation
    let mut tbs =
        match RainfallSensor::new(rain_i2c, RAINFALL_SENSOR_ADDR).await {
            Ok(sensor) => {
                info!("RAINFALL SENSOR initialised.");
                sensor
            },
            Err(e) => panic!("Could not intialise RAINFALL SENSOR: {:?}", e),
        };
    match tbs.init_config().await {
        Ok(_) => info!("RAINFALL SENSOR configuration success."),
        Err(err) => panic!("Failed to configure RAINFALL SENSOR: {:?}", err),
    };

    // Start radio task - sends queued transmissions and handles recieved packets
    _spawner.spawn(radio_task(radio, aqs, adc, tbs)).unwrap();
}


//======================================================================================================================
//----Network (MeshCore) related firmware down--------------------------------------------------------------------------
//======================================================================================================================

/// radio_task drains the MESHCORE_TX_BUFF when it is not empty, and calls
/// frame_handler when anything is recieved by the transceiver. Now handles
/// period broadcasts as well.
#[embassy_executor::task]
async fn radio_task(
    mut radio: SX1262,
    // mut wds: WindDirectionSensor,
    mut aqs: GasSensor<I2cShared>,          // It may be worth making a senors struct
    mut adc: AdcSensors,                    // to centralise the different sensors
    mut tbs: RainfallSensor<I2cShared>,     // for passing between functions/tasks
){
    // Initialise recieved packet buffer locally
    let mut meshcore_rx_buf = [0u8; MAX_PACKET_LEN];

    loop {
        // Set transceiver to receive mode to listen for incoming packets
        radio.lora_radio.prepare_for_rx(RxMode::Continuous, &radio.mod_params, &radio.rx_pkt_params)
            .await
            .expect("Prepare for rx (continuous mode) failed");

        // Using Either purposefully sets a race condition. Whichever function call
        // returns result first is taken. If a packet arrives for RX to process
        // while a TX is being sent out the recieved packet will be dropped.
        // This currently poses no issues as the only incoming packets of note
        // are requests and all the broadcasts send out the same data as a
        // response. However this should be investigated as a possible breakpoint.
        match select3(
            radio.lora_radio.rx(&radio.rx_pkt_params, &mut meshcore_rx_buf),
            MESHCORE_TX_BUFF.receive(),
            Timer::after(Duration::from_secs(15)), //Change back after testing 3600
        ).await {
            // Possibility 1: Packet recieved (success)
            Either3::First(Ok((len, status))) => {
                let frame_data = &meshcore_rx_buf[..len as usize];
                info!("RX {} bytes, rssi={} snr={}", len, status.rssi, status.snr);
                frame_handler(frame_data, &mut aqs, &mut adc, &mut tbs).await;
            }
            // Possibility 1: Packet recieved (failure)
            Either3::First(Err(e)) => warn!("radio rx error {:?}", defmt::Debug2Format(&e)),
            // Possibility 2: Packet in buffer ready to send out
            Either3::Second(frame) => send_frame(&mut radio, &frame).await,
            // Possibility 3: Periodic broadcast reached
            Either3::Third(()) => {
                let r = poll_all(&mut aqs, &mut adc, &mut tbs).await;
                if let Err(e) = network::send_group_text_sensor_data(&r) {
                    error!("failed to send GRP_TXT broadcast: {:?}", defmt::Debug2Format(&e));
                } else {
                    info!("public channel broadcast sent");
                }
            }
        }
    }
}


//======================================================================================================================
//----Watchdog related firmware-----------------------------------------------------------------------------------------
//======================================================================================================================

// Watchdog task replies to initial pulse within the window, then disables the
// the watchdog until a reset (loss of power)
// #[embassy_executor::task]
// async fn watchdog_task(mut watchdog: WatchdogTimer<Output<'static>, Output<'static>>) {
//     // CWD=NC, SET0=0, SET1=0: tWDL(max)=25.9ms, tWDU(min)=46.8ms
//     let mut ticker = Timer::every(Duration::from_millis(35));
//     loop {
//         ticker.next().await;
//         if watchdog.send_pulse().await.is_err() {
//             defmt::error!("watchdog pulse failed");
//         }
//     }
// }

    // OUT OF MAIN

    // Start async watchdog task - requires quick initalisation to pulse in the initial window on time.
    // _spawner.spawn(watchdog_task(watchdog)).unwrap();
