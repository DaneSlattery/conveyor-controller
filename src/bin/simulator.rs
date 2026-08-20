use blinksy::layout::{Layout2d, Shape2d, Vec2};
use blinksy::{
    ControlBuilder, layout2d,
    pattern::Pattern,
};
use blinksy::patterns::noise::NoiseParams;
use blinksy_desktop::{
    button::DesktopButton,
    driver::KeyCode,
    driver::{Desktop, DesktopError},
    time::elapsed_in_ms,
};
use conveyor_balancer::display::{DetectionGrid, GridParams};
use conveyor_balancer::sensor::{Detection, DetectionHistory};
use conveyor_balancer::{SENSOR_COUNT, HISTORY_DEPTH};
use embassy_executor::{Executor, Spawner};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;



layout2d!(
    PanelLayout,
    [Shape2d::Grid {
        start: Vec2::new(-1., -1.),
        horizontal_end: Vec2::new(1., -1.),
        vertical_end: Vec2::new(-1., 1.),
        horizontal_pixel_count: 16,
        vertical_pixel_count: 16,
        serpentine: true,
    }]
);


#[embassy_executor::task]
async fn draw_task(sign: &'static Signal<CriticalSectionRawMutex, [Detection<SENSOR_COUNT>; HISTORY_DEPTH]>) -> ! {
    // Press the space bar to change the color of the strip.
    // This example only cares about single clicks, so we set the release and hold times really short.
    let mut button = DesktopButton::new_embassy(
        Duration::from_micros(900),
        Duration::from_millis(1),
        Duration::from_millis(1),
    );
    let o = Desktop::new_2d::<PanelLayout>()
        .with_button(KeyCode::Space, &button)
        .start_async(async move |driver| {
            let mut control = ControlBuilder::new_2d()
                .with_layout::<PanelLayout, { PanelLayout::PIXEL_COUNT }>()
                // .with_pattern::<blinksy::patterns::noise::Noise2d<blinksy::patterns::noise::noise_fns::Perlin>>(NoiseParams::default())
                .with_pattern::<DetectionGrid<SENSOR_COUNT, HISTORY_DEPTH>>(GridParams::default())
                .with_driver(driver)
                .with_frame_buffer_size::<{ PanelLayout::PIXEL_COUNT }>()
                .build();

            loop {
                button.tick();
                // Note that `button.held_time()` only reports after the button is released,
                // so if the user holds the button down for a long time, it won't report that until they let go.
                // If you want to detect and action long presses sooner, you can use `button.current_holding_time()`, but note that
                // that reports repeatedly while the button is being held.
                //
                // The `button-driver` crate is capable of reporting on double and triple clicks, but we have deliberately tuned the
                // driver timing on this example to map them into singles.
                if button.is_clicked() || button.held_time().is_some() {
                    println!("Button activated! Changing color...");
                    // let new_color = Okhsv::new(rand::random(), 1.0, 1.0);
                    // control.set_pattern_params( { color: new_color });
                }
                button.reset();

                if let Some(x) = sign.try_take() {
                    println!("Signal received");
                    control.set_pattern_params(GridParams::new(x))
                    // let new_color = Okhsv::new(rand::random(), 1.0, 1.0);
                    // control.set_pattern_params(FlatParams { color: new_color });
                }

                if let Err(DesktopError::WindowClosed) = control.tick(elapsed_in_ms()) {
                    break;
                }
                Timer::after(Duration::from_millis(15)).await;
                // sleep(Duration::from_millis(16));
            }
        });

    o.await;
    loop {
        Timer::after(Duration::from_millis(15)).await;

        // embassy_time::Delay.delay_ms(500);
    }
    // loop{
    //
    // }
}



#[embassy_executor::task]
async fn main_task(spawner: Spawner) {
    static TRIGGER: Signal<CriticalSectionRawMutex, [Detection<SENSOR_COUNT>; HISTORY_DEPTH]> = Signal::new();
    spawner.spawn(draw_task(&TRIGGER).unwrap());
    spawner.spawn(push_task(&TRIGGER).unwrap());
}

#[embassy_executor::task]
async fn push_task(sign: &'static Signal<CriticalSectionRawMutex, [Detection<SENSOR_COUNT>; HISTORY_DEPTH]>) -> ! {
    let mut my_detections:DetectionHistory<{SENSOR_COUNT}, HISTORY_DEPTH> = DetectionHistory::new();
    // my_detections.get_history_capacity()
    let max: usize = my_detections.get_history_capacity();
    let mut count = 0;
    loop {
        Timer::after(Duration::from_millis(1000)).await;

        let mut detection: Detection<SENSOR_COUNT> = Detection::default();
        detection.iter_mut().enumerate().for_each(|(i, d)| {
            if i <= count {
                *d = true;
            } else {
                *d = false;
            }
        });
        count = (count + 1) % max;
        // detection.iter_mut().enumerate(|s,i| )
        my_detections.push_detection(detection);

        sign.signal(my_detections.detection_array());
    }
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();
fn main() {
    let executor = EXECUTOR.init(Executor::new());

    executor.run(|spawner|
        {
            spawner.spawn(main_task(spawner).unwrap())
        })
}