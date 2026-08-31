#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use blinksy::driver::ClocklessDriver;
use blinksy::layout::Layout2d;
use blinksy::leds::Ws2812;
use blinksy::{ControlBuilder, layout2d};
use blinksy_esp::ClocklessRmtBuilder;
use blinksy_esp::rmt::rmt_buffer_size;
use conveyor_balancer::sensor::{
    ArraySide, ConveyorSensor, ConveyorSensorArray,  DetectionHistory, EndStopSensor,
    median_filter, score,
};
use conveyor_balancer::stepper_motor::StepperMotor;
use core::cmp::Ordering;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Ticker, Timer};


use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{DriveMode, InputConfig, Level, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use log::{error, info, warn};
use moving_median::MovingMedian;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

use conveyor_balancer::{AppDetection, AppHistory, SensorPayload, GEAR_RATIO, GRID, HISTORY_DEPTH, MAX_OUTPUT_ANGLE, OUTPUT_ANGLE_PER_SENSOR, SENSOR_COUNT, STEPS_PER_DEGREE_INPUT, STEPS_PER_REVOLUTION, STEPS_PER_DEGREE_OUTPUT};

use crate::SteeringControlMode::{Automatic, Homing, Jogging};
use conveyor_balancer::display::{DetectionGrid, GridParams};
use conveyor_balancer::stepper_motor::Direction::{Clockwise, CounterClockwise};
use embassy_sync::watch::{Receiver, Sender, Watch};

use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use serde::Serialize;

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

    let in1 = peripherals.GPIO36;
    let in2 = peripherals.GPIO39;
    let in3 = peripherals.GPIO34;
    let in4 = peripherals.GPIO35;
    let in5 = peripherals.GPIO33;
    let in6 = peripherals.GPIO32;
    let in7 = peripherals.GPIO25;
    let in8 = peripherals.GPIO26;
    let in9 = peripherals.GPIO27;
    let in10 = peripherals.GPIO14;
    let in11 = peripherals.GPIO23;
    let in12 = peripherals.GPIO13;

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
        in1,
        in2,
        in3,
        in4,
        in5,
        in6,
        in7,
        in8,
        in9,
        in10,
    );
    println!("Number of sensors: {}", sensor_array.get_num_sensors());
    assert_eq!(sensor_array.get_num_sensors(), SENSOR_COUNT);

    let output_config =
        esp_hal::gpio::OutputConfig::default().with_drive_mode(DriveMode::OpenDrain);
    let pulse_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO19, Level::Low, output_config.clone());
    let direction_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO17, Level::Low, output_config.clone());
    let enable_pin =
        esp_hal::gpio::Output::new(peripherals.GPIO21, Level::Low, output_config.clone());

    let center_pin = esp_hal::gpio::Input::new(in11, input_config.clone());
    let aux_pin = esp_hal::gpio::Input::new(in12, input_config.clone());
    let center_sensor = EndStopSensor::new(center_pin);
    let aux_sensor = EndStopSensor::new(aux_pin);
    let stepper_driver = StepperMotor::<esp_hal::gpio::Output<'static>, embassy_time::Delay>::new(
        pulse_pin,
        direction_pin,
        enable_pin,
        embassy_time::Delay,
        STEPS_PER_REVOLUTION,
    );

    layout2d!(Layout, [GRID]);

    let led_pin = peripherals.GPIO16;
    let freq = Rate::from_mhz(80);

    let rmt = Rmt::new(peripherals.RMT, freq).unwrap();
    let driver = ClocklessDriver::default().with_led::<Ws2812>().with_writer(
        ClocklessRmtBuilder::default()
            .with_led::<Ws2812>()
            .with_rmt_buffer_size::<{ rmt_buffer_size::<Ws2812>(Layout::PIXEL_COUNT) }>()
            .with_channel(rmt.channel0)
            .with_pin(led_pin)
            .build(),
    );

    let mut _control = ControlBuilder::new_2d()
        .with_layout::<Layout, { Layout::PIXEL_COUNT }>()
        // .with_pattern::<blinksy::patterns::noise::Noise2d<blinksy::patterns::noise::noise_fns::Perlin>>(NoiseParams::default())
        .with_pattern::<DetectionGrid<SENSOR_COUNT, HISTORY_DEPTH>>(GridParams::default())
        .with_driver(driver)
        .with_frame_buffer_size::<{ Ws2812::frame_buffer_size(Layout::PIXEL_COUNT) }>()
        .build();

    static SCORE: Watch<CriticalSectionRawMutex, SensorPayload, 2> = Watch::new();
    let score_sender: Sender<CriticalSectionRawMutex, SensorPayload, 2> = SCORE.sender();

    let score_recv: Receiver<CriticalSectionRawMutex, SensorPayload, 2> = SCORE.receiver().unwrap();

    static MOTION_CONTROL: Watch<CriticalSectionRawMutex, MotionCommand, 2> = Watch::new();
    let motion_sender: Sender<CriticalSectionRawMutex, MotionCommand, 2> = MOTION_CONTROL.sender();

    let motion_recv: Receiver<CriticalSectionRawMutex, MotionCommand, 2> =
        MOTION_CONTROL.receiver().unwrap();

    // board has onboard led, will blink for some diagnostics
    let d2_led = esp_hal::gpio::Output::new(peripherals.GPIO2, Level::Low, output_config.clone());

    let spawner = spawner;
    spawner.spawn(
        measure_array(
            sensor_array,
            d2_led,
            center_sensor,
            aux_sensor,
            score_sender,
        )
        .unwrap(),
    );
    spawner.spawn(steering_control(score_recv, motion_sender).unwrap());
    spawner.spawn(run_stepper(stepper_driver, motion_recv).unwrap());

    let score_recv: Receiver<CriticalSectionRawMutex, SensorPayload, 2> = SCORE.receiver().unwrap();
    let motion_recv: Receiver<CriticalSectionRawMutex, MotionCommand, 2> =
        MOTION_CONTROL.receiver().unwrap();
    spawner.spawn(print_logs(score_recv, motion_recv).unwrap());
    // run stepper
    loop {
        Timer::after(Duration::from_millis(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}

#[embassy_executor::task]
async fn measure_array(
    mut conveyor_sensor_array: ConveyorSensorArray<esp_hal::gpio::Input<'static>, SENSOR_COUNT>,
    mut d2_led: esp_hal::gpio::Output<'static>,
    mut center_sensor: EndStopSensor<esp_hal::gpio::Input<'static>>,
    mut aux_sensor: EndStopSensor<esp_hal::gpio::Input<'static>>,
    signal: Sender<'static, CriticalSectionRawMutex, SensorPayload, 2>,
) {
    println!("Starting array measurement loop");

    let mut detection_history: AppHistory = DetectionHistory::new();

    let mut moving_median = MovingMedian::new();
    let mut ticker = Ticker::every(Duration::from_millis(500));
    loop {
        d2_led.set_high();
        match conveyor_sensor_array.sample() {
            Ok(x) => {
                let center = center_sensor.triggered().unwrap();
                let aux = aux_sensor.triggered().unwrap();
                detection_history.push_detection(x);
                let score = score(&x, conveyor_sensor_array.array_side());

                let filtered_score = median_filter(score, &mut moving_median);

                let payload = SensorPayload {
                    raw_score: score,
                    score: filtered_score.expect("No filtered score"),
                    center,
                    detections: x.clone(),
                    aux_input: aux,
                };
                signal.send(payload);
            }
            Err(x) => {
                error!("{}", x);
            }
        }
        d2_led.set_low();

        ticker.next().await;
    }
}

#[derive(Debug)]
enum SteeringControlMode {
    Homing { bounce: bool },
    Automatic,
    Jogging,
}

#[embassy_executor::task]
async fn steering_control(
    mut signal: Receiver<'static, CriticalSectionRawMutex, SensorPayload, 2>,
    motion_sender: Sender<'static, CriticalSectionRawMutex, MotionCommand, 2>,
) {
    let mut sensor_payload: SensorPayload = signal.changed().await;
    let mut bounce_off_trigger = false;
    if sensor_payload.center {
        // currently triggered, go counter clockwise until it's untriggered
        bounce_off_trigger = true;
    }
    let mut state: SteeringControlMode = Homing {
        bounce: bounce_off_trigger,
    };

    // last sensor payload

    loop {
        sensor_payload = signal.changed().await;

        let command = match &state {
            Homing { bounce } => {
                match (sensor_payload.center, bounce) {
                    (true, false) => {
                        // found the center

                        state = Automatic;
                        MotionCommand::ZeroController
                    }
                    (true, true) => {
                        // currently bouncing, keep going
                        MotionCommand::MoveAt { velocity: -1 }
                    }
                    (false, true) => {
                        // we were bouncing, now the middle is out, so reverse
                        state = Homing { bounce: false };
                        MotionCommand::Stop
                    }
                    (false, false) => {
                        // not in the middle, and not bouncing,
                        // neither triggered, keep going
                        MotionCommand::MoveAt { velocity: 1 }
                    }
                }
            }
            Automatic => {
                if sensor_payload.aux_input {
                    state = Jogging;
                    continue;
                }
                let score = sensor_payload.score;
                let roller_angle = score_to_roller_angle(score);
                let stepper_angle = roller_angle_to_stepper_angle(roller_angle);
                let step_target = stepper_angle_to_steps(stepper_angle);
                MotionCommand::MoveTo {
                    position: step_target,
                }
            }
            Jogging => {
                if !sensor_payload.aux_input {
                    state = Automatic;
                    MotionCommand::Stop
                } else {
                    MotionCommand::MoveAt { velocity: 1600 }
                }
            }
        };
        motion_sender.send(command);
    }
}

#[embassy_executor::task]
async fn run_stepper(
    mut stepper_driver: StepperMotor<esp_hal::gpio::Output<'static>, embassy_time::Delay>,
    mut recv: Receiver<'static, CriticalSectionRawMutex, MotionCommand, 2>,
) {
    println!("Starting stepper control loop");

    println!("Arming Motor...");
    stepper_driver.enable_driver().unwrap();
    println!("Motor Armed");

    let mut position = 0;

    let mut stepper_cmd = MotionCommand::Stop;
    'outer: loop {
        if let Some(new_command) = recv.try_get() {
            stepper_cmd = new_command;
        }

        match stepper_cmd {
            MotionCommand::Stop => {
                // do nothing, position hold.
                recv.changed().await;
            }
            MotionCommand::MoveTo { position: setpoint } => {
                let error = setpoint - position;
                if error == 0 {
                    stepper_cmd = recv.changed().await;
                    continue 'outer;
                }

                let direction = if error > 0 {
                    Clockwise
                } else {
                    CounterClockwise
                };

                stepper_driver.set_direction(direction).await.unwrap();

                const MAX_SPEED_SPS: u32 = 1600;
                let step_period_us = 1_000_000u64 / MAX_SPEED_SPS as u64;
                let period = step_period_us;

                stepper_driver.step().await.unwrap();
                Timer::after(Duration::from_micros(
                    period.saturating_sub(stepper_driver.step_delay()),
                ))
                .await;

                match setpoint.cmp(&position) {
                    Ordering::Less => position -= 1,
                    Ordering::Equal => {}
                    Ordering::Greater => position += 1,
                }
            }
            MotionCommand::MoveAt { velocity } => {
                if velocity == 0 {
                    stepper_cmd = MotionCommand::Stop;
                    continue 'outer;
                }
                let direction = if velocity > 0 {
                    Clockwise
                } else {
                    CounterClockwise
                };

                let speed = velocity.unsigned_abs();
                let step_period_us = 1_000_000u64 / speed as u64;

                stepper_driver.set_direction(direction).await.unwrap();

                stepper_driver.step().await.unwrap();
                Timer::after(Duration::from_micros(
                    step_period_us.saturating_sub(stepper_driver.step_delay()),
                ))
                .await;

                match direction {
                    Clockwise => position += 1,
                    CounterClockwise => position -= 1,
                }
            }
            MotionCommand::ZeroController => {
                position = 0;
                recv.changed().await;
            }
        }
    }
}

const fn score_to_roller_angle(score: i16) -> f32 {
    let roller_angle = OUTPUT_ANGLE_PER_SENSOR * score as f32;
    if score > 0 {
        roller_angle.min(MAX_OUTPUT_ANGLE)
    } else {
        roller_angle.max(-MAX_OUTPUT_ANGLE)
    }
}
const fn stepper_angle_to_steps(stepper_angle: f32) -> i32 {
    (stepper_angle * STEPS_PER_DEGREE_INPUT) as i32
}
const fn roller_angle_to_stepper_angle(roller_angle: f32) -> f32 {
    roller_angle * GEAR_RATIO as f32
}

#[embassy_executor::task]
async fn print_logs(
    mut sensor_recv: Receiver<'static, CriticalSectionRawMutex, SensorPayload, 2>,
    mut motion_recv: Receiver<'static, CriticalSectionRawMutex, MotionCommand, 2>,
) {
    let mut seq = 0;
    let mut timestamp_ms;
    loop {
        let sensor_payload = sensor_recv.get().await;
        let motion = motion_recv.get().await;
        let detections = &sensor_payload.detections;
        timestamp_ms = Instant::now().as_millis();
        seq += 1;
        let score_payload = ScorePayload{
            raw: sensor_payload.raw_score,
            filtered: sensor_payload.score,
        }     ;
        let motion_payload = match motion {
            MotionCommand::Stop => MotionPayload {
                mode: "automatic",
                command: "stop",
                target_steps: None,
                target_output_deg: None,
                velocity_sps: None,
            },
            MotionCommand::MoveTo { position } => MotionPayload {
                mode: "automatic",
                command: "move_to",
                target_steps: Some(position),
                target_output_deg: Some(position as f32 / STEPS_PER_DEGREE_OUTPUT),
                velocity_sps: None,
            },
            MotionCommand::MoveAt { velocity } => MotionPayload {
                mode: "homing", // or "jogging", based on SteeringControlMode
                command: "move_at",
                target_steps: None,
                target_output_deg: None,
                velocity_sps: Some(velocity),
            },
            MotionCommand::ZeroController => MotionPayload {
                mode: "automatic",
                command: "zero_controller",
                target_steps: None,
                target_output_deg: None,
                velocity_sps: None,
            },
        };
        let serial = SerialMessage{
            msg_type: "conveyor.telemetry",

            version:1,
            seq,
            timestamp_ms,
            detections,
            score:score_payload ,
            motion: motion_payload
        };
        let log = serde_json::to_string(&serial).unwrap();
        println!("{}", log);
        // println!(
        //     "{{\"type\": \"conveyor.telemetry\",\"version\":1,\"seq\":{seq}\"timestamp_ms\":{timestamp_ms},\"detections\":{detections:?},\"score\":{{\"raw\": {},\"filtered\": {} }},\"motion\": {{\"mode\":\"automatic\",\"command\":{:?}} }}",
        //     sensor_payload.raw_score, sensor_payload.score, motion
        // );
        // println!("sensor: {:?}", sensor_recv.recv().await);
        // println!("motion: {:?}", motion_recv.recv().await);
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[derive(Debug,Serialize)]
struct SerialMessage<'a>{
    #[serde(rename="type")]
  pub    msg_type: &'static str,
    pub version: u32,
    pub seq: u32,
    pub timestamp_ms: u64,
    pub detections: &'a AppDetection,
    pub score: ScorePayload,
    pub motion: MotionPayload,
}
#[derive(Debug,Serialize)]

struct MotionPayload{
    mode: &'static str,
    command: &'static str,
    target_steps: Option<i32>,
    target_output_deg: Option<f32>,
    velocity_sps: Option<i32>
}
#[derive(Debug,Serialize)]

struct ScorePayload{
    raw: i16,
    filtered: i16,
}
#[derive(Clone, Debug)]
enum MotionCommand {
    Stop,
    MoveTo { position: i32 },
    MoveAt { velocity: i32 },
    ZeroController,
}
