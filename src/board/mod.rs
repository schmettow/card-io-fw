pub mod drivers;
pub mod initialized;
pub mod startup;
pub mod utils;

use esp_backtrace as _;

#[cfg(feature = "esp32s2")]
pub use esp32s2_hal as hal;

#[cfg(feature = "esp32s3")]
pub use esp32s3_hal as hal;

#[cfg(feature = "esp32s2")]
pub use esp32s2 as pac;

#[cfg(feature = "esp32s3")]
pub use esp32s3 as pac;
use gui::screens::display_menu::BatteryDisplayStyle;
use signal_processing::battery::BatteryModel;

use display_interface_spi::SPIInterface;
use drivers::{
    battery_adc::BatteryAdc as BatteryAdcType,
    display::{Display as DisplayType, PoweredDisplay as PoweredDisplayType},
    frontend::{Frontend, PoweredFrontend},
};
use hal::{
    adc::ADC2,
    dma::{ChannelRx, ChannelTx},
    gdma::*,
    gpio::{
        Analog, Bank0GpioRegisterAccess, Bank1GpioRegisterAccess, Floating, GpioPin, Input,
        InputOutputAnalogPinType, InputOutputPinType, InteruptStatusRegisterAccessBank0,
        InteruptStatusRegisterAccessBank1, Output, PullUp, PushPull,
    },
    soc::gpio::*,
    spi::{dma::SpiDma, FullDuplexMode},
};
use utils::{DummyOutputPin, SpiDeviceWrapper};

pub type DisplaySpi<'d> = SpiDeviceWrapper<
    SpiDma<
        'd,
        hal::peripherals::SPI2,
        ChannelTx<'d, Channel0TxImpl, Channel0>,
        ChannelRx<'d, Channel0RxImpl, Channel0>,
        SuitablePeripheral0,
        FullDuplexMode,
    >,
    DummyOutputPin,
>;

pub type DisplayDataCommand = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio13Signals,
    13,
>;
pub type DisplayChipSelect = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio10Signals,
    10,
>;
pub type DisplayReset = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio9Signals,
    9,
>;

pub type DisplayInterface<'a> = SPIInterface<DisplaySpi<'a>, DisplayDataCommand>;

pub type AdcDrdy = GpioPin<
    Input<Floating>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio4Signals,
    4,
>;
pub type AdcReset = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio2Signals,
    2,
>;
pub type TouchDetect = GpioPin<
    Input<Floating>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio1Signals,
    1,
>;
pub type AdcChipSelect = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio18Signals,
    18,
>;
pub type AdcSpi<'d> = SpiDeviceWrapper<
    SpiDma<
        'd,
        hal::peripherals::SPI3,
        ChannelTx<'d, Channel1TxImpl, Channel1>,
        ChannelRx<'d, Channel1RxImpl, Channel1>,
        SuitablePeripheral1,
        FullDuplexMode,
    >,
    AdcChipSelect,
>;

pub type BatteryAdcInput = GpioPin<
    Analog,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio17Signals,
    17,
>;
pub type BatteryAdcEnable = GpioPin<
    Output<PushPull>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio8Signals,
    8,
>;
pub type VbusDetect = GpioPin<
    Input<Floating>,
    Bank1GpioRegisterAccess,
    InteruptStatusRegisterAccessBank1,
    InputOutputPinType,
    Gpio47Signals,
    47,
>;
pub type ChargeCurrentInput = GpioPin<
    Analog,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio14Signals,
    14,
>;
pub type ChargerStatus = GpioPin<
    Input<PullUp>,
    Bank0GpioRegisterAccess,
    InteruptStatusRegisterAccessBank0,
    InputOutputAnalogPinType,
    Gpio21Signals,
    21,
>;

pub type EcgFrontend = Frontend<AdcSpi<'static>, AdcDrdy, AdcReset, TouchDetect>;
pub type PoweredEcgFrontend = PoweredFrontend<AdcSpi<'static>, AdcDrdy, AdcReset, TouchDetect>;

pub type Display = DisplayType<DisplayInterface<'static>, DisplayReset>;
pub type PoweredDisplay = PoweredDisplayType<DisplayInterface<'static>, DisplayReset>;

pub type BatteryAdc = BatteryAdcType<BatteryAdcInput, ChargeCurrentInput, BatteryAdcEnable, ADC2>;

pub struct MiscPins {
    pub vbus_detect: VbusDetect,
    pub chg_status: ChargerStatus,
}

pub const BATTERY_MODEL: BatteryModel = BatteryModel {
    voltage: (2750, 4200),
    charge_current: (0, 1000),
};

pub const LOW_BATTERY_VOLTAGE: u16 = 3300;

pub const DEFAULT_BATTERY_DISPLAY_STYLE: BatteryDisplayStyle = BatteryDisplayStyle::Indicator;
