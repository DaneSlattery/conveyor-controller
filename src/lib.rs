#![no_std]
//! A conveyor belt active alignment control system.
//!
//! An active feedback controller is used to drive a conveyor belt to be centered using a
//! steering system.
//!
//! The system reads an array of digital inputs
//! [0,1,2,3...N]
//!
//! Each digital input represents conveyor belt alignment. If the conveyor is aligned
//! half the inputs are active and half are inactive, so it is more accurate to model as
//! [-N,... -2,1,0,1,2,...N]
//!
//! As the conveyor shifts alignment, the inputs will become more negative or more positive.

extern crate alloc;

pub mod sensor;
pub mod shaft_position {}

pub mod controller;

pub mod stepper_motor;

pub mod display;

pub const SENSOR_COUNT: usize = 10;
pub const HISTORY_DEPTH: usize = 11;

pub const STEPS_PER_REVOLUTION: StepsPerRevolution = StepsPerRevolution::Steps1600;

/// Number of input turns per output turn.
pub const GEAR_RATIO: u16 = 100;

/// Maximum angle of the output shaft in degrees
pub const MAX_OUTPUT_ANGLE: f32 = 6.;

/// Number of steps to move the input shaft by 1 degree
pub const STEPS_PER_DEGREE_INPUT: f32 = STEPS_PER_REVOLUTION.steps_per_revolution() as f32 / 360.0;

/// Number of steps to move the output shaft by 1 degree
pub const STEPS_PER_DEGREE_OUTPUT: f32 = STEPS_PER_DEGREE_INPUT * GEAR_RATIO as f32;

/// The angle to move the output shaft per triggered sensor
pub const OUTPUT_ANGLE_PER_SENSOR: f32 = MAX_OUTPUT_ANGLE / (SENSOR_COUNT / 2) as f32;

/// Minimum error for angles
pub const ANGLE_EPSILON: f32 = 0.1;

pub const MEDIAN_FILTER_SIZE: usize = 20;
pub const PANEL_WIDTH: usize = 16;
pub const PANEL_HEIGHT: usize = 16;
pub const PANEL_PIXELS: usize = PANEL_WIDTH * PANEL_HEIGHT;

// Then add application-level type aliases:

use crate::display::{DetectionGrid, GridParams};
use crate::sensor::{Detection, DetectionHistory};
use crate::stepper_motor::StepsPerRevolution;
use blinksy::layout::{Shape2d, Vec2};

pub type AppDetection = Detection<SENSOR_COUNT>;
pub type AppHistory = DetectionHistory<SENSOR_COUNT, HISTORY_DEPTH>;
pub type AppFrame = [AppDetection; HISTORY_DEPTH];

pub type AppGrid = DetectionGrid<SENSOR_COUNT, HISTORY_DEPTH>;
pub type AppGridParams = GridParams<SENSOR_COUNT, HISTORY_DEPTH>;

pub const GRID: Shape2d = Shape2d::Grid {
    start: Vec2::new(-1., -1.),
    horizontal_end: Vec2::new(1., -1.),
    vertical_end: Vec2::new(-1., 1.),
    horizontal_pixel_count: 16,
    vertical_pixel_count: 16,
    serpentine: true,
};

pub struct SensorPayload {
    pub score: i16,
    pub center: bool,
    pub aux_input: bool,
}
