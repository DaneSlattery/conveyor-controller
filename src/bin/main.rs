#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use blinksy::driver::ClocklessDriver;
use blinksy::layout::{Shape2d, Vec2};
use blinksy::leds::Ws2812;
use blinksy::layout::Layout2d;
use blinksy::{ControlBuilder, layout2d};
use blinksy::patterns::noise::NoiseParams;
use blinksy_esp::ClocklessRmtBuilder;
use blinksy_esp::rmt::rmt_buffer_size;
use blinksy_esp::time::elapsed;
use conveyor_balancer::sensor::{median_filter, score, ArraySide, ConveyorSensor, ConveyorSensorArray, Detection, DetectionHistory};
use conveyor_balancer::stepper_motor::{Direction, StepperMotor, StepsPerRevolution};
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker, Timer};
// use embedded_hal::delay::DelayNs;

use embedded_hal_async::delay::DelayNs;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{DriveMode, InputConfig, Level, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::xtensa_lx::timer::delay;
use esp_println::println;
use log::{error, info};
use moving_median::MovingMedian;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

use conveyor_balancer::{AppHistory, GRID, SENSOR_COUNT, HISTORY_DEPTH};

use embassy_sync::signal;
use embassy_sync::signal::Signal;
use embassy_sync::watch::{Sender, Watch};
use embedded_hal::digital::OutputPin;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use conveyor_balancer::display::{DetectionGrid, GridParams};

macro_rules! sensor_array    {
    ($config:expr,$side:expr,$($pin:expr),+$(,)?) => {
        ConveyorSensorArray::new(
            [$(
                 ConveyorSensor::new(
                     esp_hal::gpio::Input::new(
                         $pin,
                         $config.clone()
                     )
                 )),+
            ],
        $side)
    };
}

// const NUM_SENSORS: usize = 10;

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32 -o esp32-wroom-32 -o alloc -o unstable-hal -o embassy -o log -o esp-backtrace -o wokwi -o ci

    esp_println::logger::init_logger_from_env();
    println!("Starting main");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // The following pins are used to bootstrap the chip. They are available
    // for use, but check the datasheet of the module for more information on them.
    // - GPIO0
    // - GPIO2
    // - GPIO5
    // - GPIO12
    // - GPIO15
    // These GPIO pins are in use by some feature of the module and should not be used.
    let _ = peripherals.GPIO6;
    let _ = peripherals.GPIO7;
    let _ = peripherals.GPIO8;
    let _ = peripherals.GPIO9;
    let _ = peripherals.GPIO10;
    let _ = peripherals.GPIO11;
    let _ = peripherals.GPIO16;
    let _ = peripherals.GPIO20;

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 98768);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");
    let input_config = InputConfig::default().with_pull(Pull::Up);

    let sensor_array = sensor_array!(
        input_config,
        ArraySide::Right,
        peripherals.GPIO23,
        peripherals.GPIO22,
        peripherals.GPIO26,
        peripherals.GPIO25,
        peripherals.GPIO21,
        peripherals.GPIO19,
        peripherals.GPIO18,
        peripherals.GPIO5,
        peripherals.GPIO17,
        peripherals.GPIO15,
    );
    println!("Number of sensors: {}", sensor_array.get_num_sensors());
    assert_eq!(sensor_array.get_num_sensors(), SENSOR_COUNT);

    let output_config = esp_hal::gpio::OutputConfig::default().with_drive_mode(DriveMode::PushPull);
    let pulse_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO14, Level::Low, output_config.clone());
    let direction_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO27, Level::Low, output_config.clone());
    let enable_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO13, Level::Low, output_config.clone());
    let stepper_driver = StepperMotor::<esp_hal::gpio::Output<'static>, esp_hal::delay::Delay>::new(
        pulse_pin,
        direction_pin,
        enable_pin,
        esp_hal::delay::Delay::new(),
        StepsPerRevolution::Steps800,
    );

    layout2d!(Layout, [GRID]);

    let led_pin = peripherals.GPIO16;
    let freq = Rate::from_mhz(80);

    let rmt = Rmt::new(peripherals.RMT, freq).unwrap();
    let driver = ClocklessDriver::default()
        .with_led::<Ws2812>()
        .with_writer(
        ClocklessRmtBuilder::default()
            .with_led::<Ws2812>()
            .with_rmt_buffer_size::<{rmt_buffer_size::<Ws2812>(Layout::PIXEL_COUNT)}>()
            .with_channel(rmt.channel0)
            .with_pin(led_pin)
            .build(),
    );

    let mut control = ControlBuilder::new_2d()
        .with_layout::<Layout, { Layout::PIXEL_COUNT }>()
        // .with_pattern::<blinksy::patterns::noise::Noise2d<blinksy::patterns::noise::noise_fns::Perlin>>(NoiseParams::default())
        .with_pattern::<DetectionGrid<SENSOR_COUNT, HISTORY_DEPTH>>(GridParams::default())
        .with_driver(driver)
        .with_frame_buffer_size::<{ Ws2812::frame_buffer_size(Layout::PIXEL_COUNT) }>()
        .build();

    static SCORE_SIGNAL: Signal<CriticalSectionRawMutex, i16> = Signal::new();
    static score_watch: Watch<CriticalSectionRawMutex,[Detection<SENSOR_COUNT>; HISTORY_DEPTH],1> = Watch::new();
    let sender :Sender<CriticalSectionRawMutex,[Detection<SENSOR_COUNT>; HISTORY_DEPTH],1>= score_watch.sender();
    // board has onboard led, will blink for some diagnostics
    let d2_led = esp_hal::gpio::Output::new(peripherals.GPIO2, Level::Low, output_config.clone());

    let spawner = spawner;
    spawner.spawn(measure_array(sensor_array, d2_led, &SCORE_SIGNAL,sender).unwrap());
    spawner.spawn(run_stepper(stepper_driver, &SCORE_SIGNAL).unwrap());
    // run stepper
    control.set_brightness(0.01);
    loop {
        let elapsed_in_ms = elapsed().as_millis();
        let detections = score_watch.anon_receiver().try_get();
        if let Some(d) = detections{
        control.set_pattern_params(GridParams::new(d));

        }
        control.tick(elapsed_in_ms).unwrap();
        Timer::after(Duration::from_millis(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}

#[embassy_executor::task]
async fn measure_array(
    mut conveyor_sensor_array: ConveyorSensorArray<esp_hal::gpio::Input<'static>, SENSOR_COUNT>,
    mut d2_led: esp_hal::gpio::Output<'static>,
    signal: &'static Signal<CriticalSectionRawMutex, i16>,
    mut sender:  Sender<'static, CriticalSectionRawMutex,[Detection<SENSOR_COUNT>; HISTORY_DEPTH],1>
) {
    // todo: transmit score to another thread
    println!("Starting array measurement loop");

    let mut detection_history: AppHistory = DetectionHistory::new();

    let mut moving_median = MovingMedian::new();
    let mut ticker = Ticker::every(Duration::from_millis(1000));
    loop {
        d2_led.set_high();
        match conveyor_sensor_array.sample() {
            Ok(x) => {
                detection_history.push_detection(x);
                sender.send(detection_history.detection_array());
                let score = score(&x, conveyor_sensor_array.array_side());

                let filtered_score = median_filter(score, &mut moving_median);

                // todo: add noise suppression, filter score and software-debounce inputs
                println!(
                    "Detections: {:?}, Raw Score: {}, filtered score: {:?}",
                    x, score, filtered_score
                );
                // println!("Detections: {:?}, Score: {}", x, score);

                signal.signal(score);
            }
            Err(x) => {
                error!("{}", x);
            }
        }
        d2_led.set_low();

        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn run_stepper(
    mut stepper_driver: StepperMotor<esp_hal::gpio::Output<'static>, esp_hal::delay::Delay>,
    signal: &'static Signal<CriticalSectionRawMutex, i16>,
) {
    // todo: transmit score to another thread
    println!("Starting stepper control loop");

    println!("Arming Motor...");
    stepper_driver.enable_driver().unwrap();
    println!("Motor Armed");

    println!("Direction set Clockwise...");
    stepper_driver
        .set_direction(Direction::CounterClockwise)
        .unwrap();
    let mut delay = embassy_time::Delay;
    delay.delay_ms(1000).await;

    loop {
        if let Some(score) = signal.try_take() {
            if score > 0 {
                println!("CCL");

                stepper_driver.enable_driver().unwrap();
                stepper_driver
                    .set_direction(Direction::CounterClockwise)
                    .unwrap();
                stepper_driver.step().unwrap();
            } else if score < 0 {
                println!("CL");

                stepper_driver.enable_driver().unwrap();
                stepper_driver.set_direction(Direction::Clockwise).unwrap();
                stepper_driver.step().unwrap();
            } else {
                println!("Disarming Motor...");

                stepper_driver.disable_driver().unwrap();
            }
        }
        delay.delay_us(500).await;
    }

    const MAX_STEPS: u16 = 3200;

    println!("Step {MAX_STEPS} times");
}

#[embassy_executor::task]
async fn draw_task(signal: &'static Signal<CriticalSectionRawMutex, i16>, // control:
) {
    loop {
        Timer::after(Duration::from_millis(100)).await;
    }
}
