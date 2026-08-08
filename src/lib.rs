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

pub mod sensor;
pub mod shaft_position{}

pub mod controller{}

pub mod stepper_motor{}

