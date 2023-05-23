use crate::{
    board::{
        hal::{
            self,
            clock::{ClockControl, Clocks, CpuClock},
            dma::DmaPriority,
            embassy,
            gdma::Gdma,
            interrupt, peripherals,
            peripherals::Peripherals,
            prelude::*,
            spi::{
                dma::{WithDmaSpi2, WithDmaSpi3},
                SpiMode,
            },
            Rtc, Spi, IO,
        },
        *,
    },
    heap::init_heap,
    interrupt::{InterruptExecutor, SwInterrupt0},
};
use display_interface_spi::SPIInterface;
use embassy_executor::SendSpawner;
use esp_println::logger::init_logger;
use hal::systimer::SystemTimer;

static INT_EXECUTOR: InterruptExecutor<SwInterrupt0> = InterruptExecutor::new();

#[interrupt]
fn FROM_CPU_INTR0() {
    unsafe { INT_EXECUTOR.on_interrupt() }
}

pub struct StartupResources {
    pub display: Display,
    pub frontend: EcgFrontend,
    pub clocks: Clocks<'static>,
    pub battery_adc: BatteryAdc,
    pub misc_pins: MiscPins,
    pub high_prio_spawner: SendSpawner,
}

impl StartupResources {
    pub fn initialize() -> StartupResources {
        init_heap();
        init_logger(log::LevelFilter::Debug);

        let peripherals = Peripherals::take();

        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock240MHz).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        rtc.rwdt.disable();

        embassy::init(&clocks, SystemTimer::new(peripherals.SYSTIMER));

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        let dma = Gdma::new(peripherals.DMA, &mut system.peripheral_clock_control);

        // Display
        let display_dma_channel = dma.channel0;
        interrupt::enable(
            peripherals::Interrupt::DMA_IN_CH0,
            interrupt::Priority::Priority1,
        )
        .unwrap();
        interrupt::enable(
            peripherals::Interrupt::DMA_OUT_CH0,
            interrupt::Priority::Priority1,
        )
        .unwrap();

        let display_reset = io.pins.gpio9.into_push_pull_output();
        let display_dc = io.pins.gpio13.into_push_pull_output();

        let mut display_cs: DisplayChipSelect = io.pins.gpio10.into_push_pull_output();
        let display_sclk = io.pins.gpio12;
        let display_mosi = io.pins.gpio11;

        let display_spi = peripherals.SPI2;

        display_cs.connect_peripheral_to_output(display_spi.cs_signal());

        static mut DISPLAY_SPI_DESCRIPTORS: [u32; 24] = [0u32; 8 * 3];
        static mut DISPLAY_SPI_RX_DESCRIPTORS: [u32; 24] = [0u32; 8 * 3];
        let display_spi = Spi::new_no_cs_no_miso(
            display_spi,
            display_sclk,
            display_mosi,
            40u32.MHz(),
            SpiMode::Mode0,
            &mut system.peripheral_clock_control,
            &clocks,
        )
        .with_dma(display_dma_channel.configure(
            false,
            unsafe { &mut DISPLAY_SPI_DESCRIPTORS },
            unsafe { &mut DISPLAY_SPI_RX_DESCRIPTORS },
            DmaPriority::Priority0,
        ));

        let display = Display::new(
            SPIInterface::new(
                SpiDeviceWrapper::new(display_spi, DummyOutputPin),
                display_dc,
            ),
            display_reset,
        );

        // ADC
        let adc_dma_channel = dma.channel1;
        interrupt::enable(
            peripherals::Interrupt::DMA_IN_CH1,
            interrupt::Priority::Priority2,
        )
        .unwrap();
        interrupt::enable(
            peripherals::Interrupt::DMA_OUT_CH1,
            interrupt::Priority::Priority2,
        )
        .unwrap();

        let adc_sclk = io.pins.gpio6;
        let adc_mosi = io.pins.gpio7;
        let adc_miso = io.pins.gpio5;

        let adc_drdy = io.pins.gpio4.into_floating_input();
        let adc_reset = io.pins.gpio2.into_push_pull_output();
        let touch_detect = io.pins.gpio1.into_floating_input();
        let mut adc_cs = io.pins.gpio18.into_push_pull_output();

        adc_cs.set_high().unwrap();

        static mut ADC_SPI_DESCRIPTORS: [u32; 24] = [0u32; 8 * 3];
        static mut ADC_SPI_RX_DESCRIPTORS: [u32; 24] = [0u32; 8 * 3];
        let adc = Frontend::new(
            SpiDeviceWrapper::new(
                Spi::new_no_cs(
                    peripherals.SPI3,
                    adc_sclk,
                    adc_mosi,
                    adc_miso,
                    1u32.MHz(),
                    SpiMode::Mode1,
                    &mut system.peripheral_clock_control,
                    &clocks,
                )
                .with_dma(adc_dma_channel.configure(
                    false,
                    unsafe { &mut ADC_SPI_DESCRIPTORS },
                    unsafe { &mut ADC_SPI_RX_DESCRIPTORS },
                    DmaPriority::Priority1,
                )),
                adc_cs,
            ),
            adc_drdy,
            adc_reset,
            touch_detect,
        );

        // Battery measurement
        let batt_adc_in = io.pins.gpio17.into_analog();
        let batt_adc_en = io.pins.gpio8.into_push_pull_output();

        // Charger
        let vbus_detect = io.pins.gpio47.into_floating_input();
        let chg_current = io.pins.gpio14.into_analog();
        let chg_status = io.pins.gpio21.into_pull_up_input();

        let high_prio_spawner = INT_EXECUTOR.start();

        // Battery ADC
        let analog = peripherals.SENS.split();

        let battery_adc = BatteryAdc::new(analog.adc2, batt_adc_in, chg_current, batt_adc_en);

        StartupResources {
            display,
            frontend: adc,
            clocks,
            battery_adc,
            high_prio_spawner,

            misc_pins: MiscPins {
                vbus_detect,
                chg_status,
            },
        }
    }
}
