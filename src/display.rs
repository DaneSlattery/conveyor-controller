use crate::sensor::Detection;
use blinksy::layout::{Layout2d, Shape2d};
use blinksy::markers::Dim2d;
use blinksy::pattern::Pattern;

pub struct GridParams<const NUM_SENSORS: usize, const NUM_HISTORY: usize> {
    detection_history: [Detection<NUM_SENSORS>; NUM_HISTORY],
}

impl<const NUM_SENSORS: usize, const NUM_HISTORY: usize> GridParams<NUM_SENSORS, NUM_HISTORY> {
    pub fn new(detection: [Detection<NUM_SENSORS>; NUM_HISTORY]) -> Self {
        Self {
            detection_history: detection,
        }
    }
}

impl<const NUM_SENSORS: usize, const NUM_HISTORY: usize> Default
    for GridParams<NUM_SENSORS, NUM_HISTORY>
{
    fn default() -> Self {
        Self {
            detection_history: [[false; NUM_SENSORS]; NUM_HISTORY],
        }
    }
}

pub struct DetectionGrid<const NUM_SENSORS: usize, const NUM_HISTORY: usize> {
    params: GridParams<NUM_SENSORS, NUM_HISTORY>,
}

pub enum DetectionGridColors {
    Detection,
    NoDetection,
    Border,
}

impl<Layout, const NUM_SENSORS: usize, const NUM_HISTORY: usize> Pattern<Dim2d, Layout>
    for DetectionGrid<NUM_SENSORS, NUM_HISTORY>
where
    Layout: Layout2d,
{
    type Params = GridParams<NUM_SENSORS, NUM_HISTORY>;
    type Color = blinksy::color::Srgb;

    fn new(params: Self::Params) -> Self {
        Self { params }
    }

    fn tick(&self, _time_in_ms: u64) -> impl Iterator<Item = Self::Color> {
        // let active = ((_time_in_ms / 250) as usize) % Layout::PIXEL_COUNT;
        //
        // return (0..Layout::PIXEL_COUNT).map(move |index| {
        //     if index == active {
        //         Self::Color::new(1.0, 0.0, 0.0)
        //     } else {
        //         Self::Color::new(0.0, 0.0, 0.0)
        //     }
        // });

        let shape = Layout::shapes()
            .next()
            .expect("DetectionGrid requires a layout shape");

        let Shape2d::Grid {
            horizontal_pixel_count: width,
            vertical_pixel_count: height,
            serpentine,
            ..
        } = shape
        else {
            panic!("DetectionGrid requires a grid layout");
        };

        assert_eq!(
            Layout::PIXEL_COUNT,
            width * height,
            "DetectionGrid currently supports exactly one grid shape"
        );
        assert!(NUM_SENSORS <= width, "sensor count exceeds panel width");
        assert!(NUM_HISTORY <= height, "history depth exceeds panel height");

        let column_offset = (width - NUM_SENSORS) / 2;
        let row_offset = height - NUM_HISTORY;

        (0..Layout::PIXEL_COUNT).map(move |led_index| {
            let row = led_index / width;
            let wired_column = led_index % width;
            // log::info!("row: {}, column: {}", row, wired_column);
            // Blinksy reverses odd rows for a serpentine grid.
            let column = if serpentine && !row.is_multiple_of(2) {
                width - 1 - wired_column
            } else {
                wired_column
            };
            let sensor_column = column.checked_sub(column_offset);
            let history_row = row.checked_sub(row_offset);

            let detection_color = match (sensor_column, history_row) {
                (Some(x), Some(y)) if x < NUM_SENSORS && y < NUM_HISTORY => {
                    if self.params.detection_history[y][x] {
                        DetectionGridColors::Detection
                    } else {
                        DetectionGridColors::NoDetection
                    }
                }
                _ => DetectionGridColors::Border,
            };

            match detection_color {
                DetectionGridColors::Detection => Self::Color::new(1.0, 0.0, 0.0),
                DetectionGridColors::NoDetection => Self::Color::new(0.0, 1.0, 0.0),
                DetectionGridColors::Border => Self::Color::new(1.0, 0.0, 1.0),
            }
        })
    }

    fn set_params(&mut self, params: Self::Params) {
        self.params = params;
    }
}
