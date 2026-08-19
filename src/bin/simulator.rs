use blinksy::{
    ControlBuilder, color::Okhsv, layout::Layout1d, layout1d, markers::Dim1d, pattern::Pattern,
};
use blinksy_desktop::{
    button::DesktopButton,
    driver::KeyCode,
    driver::{Desktop, DesktopError},
    time::elapsed_in_ms,
};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use embedded_hal::delay::DelayNs;

layout1d!(StripLayout, 30);

pub struct FlatParams {
    color: Okhsv,
}

impl Default for FlatParams {
    fn default() -> Self {
        Self {
            color: Okhsv::new(0., 1.0, 1.0),
        }
    }
}

pub struct Flat(FlatParams);

impl<Layout> Pattern<Dim1d, Layout> for Flat
where
    Layout: Layout1d,
{
    type Params = FlatParams;
    type Color = Okhsv;

    fn new(params: Self::Params) -> Self {
        Self(params)
    }

    fn tick(&self, _time_in_ms: u64) -> impl Iterator<Item = Self::Color> {
        Layout::points().map(|_x| self.0.color)
    }

    fn set_params(&mut self, params: Self::Params) {
        self.0 = params;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    static TRIGGER: Signal<CriticalSectionRawMutex, bool> = Signal::new();
    spawner.spawn(draw_task(&TRIGGER).unwrap());
    spawner.spawn(push_task(&TRIGGER).unwrap());
}

#[embassy_executor::task]
async fn push_task(sign: &'static Signal<CriticalSectionRawMutex, bool>) -> ! {
    loop {
        Timer::after(Duration::from_millis(150)).await;


        sign.signal(true);
    }
}

#[embassy_executor::task]
async fn draw_task(sign: &'static Signal<CriticalSectionRawMutex, bool>) -> ! {
    // Press the space bar to change the color of the strip.
    // This example only cares about single clicks, so we set the release and hold times really short.
    let mut button = DesktopButton::new_embassy(
        Duration::from_micros(900),
        Duration::from_millis(1),
        Duration::from_millis(1),
    );
    let o = Desktop::new_1d::<StripLayout>()
        .with_button(KeyCode::Space, &button)
        .start_async(async move |driver| {
            let mut control = ControlBuilder::new_1d()
                .with_layout::<StripLayout, { StripLayout::PIXEL_COUNT }>()
                .with_pattern::<Flat>(FlatParams::default())
                .with_driver(driver)
                .with_frame_buffer_size::<{ StripLayout::PIXEL_COUNT }>()
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
                    let new_color = Okhsv::new(rand::random(), 1.0, 1.0);
                    control.set_pattern_params(FlatParams { color: new_color });
                }
                button.reset();
                
                if let Some(x)= sign.try_take(){
                    println!("Signal received");
                    
                    let new_color = Okhsv::new(rand::random(), 1.0, 1.0);
                    control.set_pattern_params(FlatParams { color: new_color });
                    
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
