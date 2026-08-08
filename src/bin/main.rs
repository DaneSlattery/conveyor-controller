#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{InputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use log::{error, info};
use conveyor_balancer::sensor::{score, ConveyorSensor, ConveyorSensorArray, ArraySide};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32 -o esp32-wroom-32 -o alloc -o unstable-hal -o embassy -o log -o esp-backtrace -o wokwi -o ci

    esp_println::logger::init_logger_from_env();

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

    let conveyor_sensor_1 = ConveyorSensor::new(esp_hal::gpio::Input::new(
        peripherals.GPIO23, // todo: map peripherals
        input_config.clone(),
    ));
    let conveyor_sensor_2 = ConveyorSensor::new(esp_hal::gpio::Input::new(
        peripherals.GPIO22, // todo: map peripherals
        input_config.clone(),
    ));
    let conveyor_sensor_3 = ConveyorSensor::new(esp_hal::gpio::Input::new(
        peripherals.GPIO19, // todo: map peripherals
        input_config.clone(),
    ));
    let conveyor_sensor_4 = ConveyorSensor::new(esp_hal::gpio::Input::new(
        peripherals.GPIO21, // todo: map peripherals
        input_config.clone(),
    ));
    // initialise N conveyor sensors
    let sensor_array = ConveyorSensorArray::new([conveyor_sensor_1,conveyor_sensor_2,conveyor_sensor_3,conveyor_sensor_4],ArraySide::Left);


    let spawner = spawner;
    spawner.spawn(measure_array(sensor_array).unwrap());
    // run stepper
    loop {
        Timer::after(Duration::from_secs(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}



#[embassy_executor::task]
async fn measure_array(mut conveyor_sensor_array: ConveyorSensorArray<esp_hal::gpio::Input<'static>,4>)
{
    // todo: transmit score to another thread
    println!("Starting array measurement loop");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop{
        match conveyor_sensor_array.sample(){
            Ok(x) => {
                let score = score(&x,conveyor_sensor_array.array_side());
                // todo: add noise suppression, filter score and software-debounce inputs
                println!("Detections: {:?}, Score: {}",x, score);
            }
            Err(x) => {
                error!("{}", x);
            }
        }
        ticker.next().await;
    }
}

