//! Stepper motor driver module.
//! The stepper motor driver in this case is a DQ542MA microstep driver
//! It has inputs for
//!  Pulse - used to step the motor
//!  Direction - used to set the direction of rotation
//!  Enable - used to enable or disable the motor

use crate::stepper_motor::Direction::{Clockwise, CounterClockwise};
use embassy_time::Timer;
use embedded_hal::digital::ErrorType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

pub enum StandStillCurrent {
    Half,
    Full,
}

pub struct DipSwitches {
    pub switch1: bool,
    pub switch2: bool,
    pub switch3: bool,
    pub switch4: bool,
    pub switch5: bool,
    pub switch6: bool,
    pub switch7: bool,
    pub switch8: bool,
}

impl DipSwitches {
    pub const fn stand_still_current(dip_switches: DipSwitches) -> StandStillCurrent {
        match dip_switches.switch4 {
            // ON => standstill current = same as selected current
            true => StandStillCurrent::Full,
            // OFF => standstill current = half of selected current
            false => StandStillCurrent::Half,
        }
    }

    pub const fn peak_current(dip_switches: DipSwitches) -> f32 {
        match (
            dip_switches.switch1,
            dip_switches.switch2,
            dip_switches.switch3,
        ) {
            (true, true, true) => 1.0,
            (false, true, true) => 1.46,
            (true, false, true) => 1.91,
            (false, false, true) => 2.37,
            (true, true, false) => 2.84,
            (false, true, false) => 3.31,
            (true, false, false) => 3.76,
            (false, false, false) => 4.2,
        }
    }

    pub const fn rms_current(dip_switches: DipSwitches) -> f32 {
        match (
            dip_switches.switch1,
            dip_switches.switch2,
            dip_switches.switch3,
        ) {
            (true, true, true) => 0.71,
            (false, true, true) => 1.04,
            (true, false, true) => 1.36,
            (false, false, true) => 1.69,
            (true, true, false) => 2.03,
            (false, true, false) => 2.36,
            (true, false, false) => 2.69,
            (false, false, false) => 3.0,
        }
    }
}

impl TryFrom<DipSwitches> for StepsPerRevolution {
    type Error = ();

    fn try_from(value: DipSwitches) -> Result<Self, Self::Error> {
        match (value.switch5, value.switch6, value.switch7, value.switch8) {
            (false, true, true, true) => Ok(Self::Steps400),
            (true, false, true, true) => Ok(Self::Steps800),
            (false, false, true, true) => Ok(Self::Steps1600),
            (true, true, false, true) => Ok(Self::Steps3200),
            (false, true, false, true) => Ok(Self::Steps6400),
            (true, false, false, true) => Ok(Self::Steps12800),
            (false, false, false, true) => Ok(Self::Steps25600),
            (true, true, true, false) => Ok(Self::Steps1000),
            (false, true, true, false) => Ok(Self::Steps2000),
            (true, false, true, false) => Ok(Self::Steps4000),
            (false, false, true, false) => Ok(Self::Steps5000),
            (true, true, false, false) => Ok(Self::Steps8000),
            (false, true, false, false) => Ok(Self::Steps10000),
            (true, false, false, false) => Ok(Self::Steps20000),
            (false, false, false, false) => Ok(Self::Steps25000),
            (true, true, true, true) => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StepsPerRevolution {
    Steps400 = 400,
    Steps800 = 800,
    Steps1600 = 1600,
    Steps3200 = 3200,
    Steps6400 = 6400,
    Steps12800 = 12800,
    Steps25600 = 25600,
    Steps1000 = 1000,
    Steps2000 = 2000,
    Steps4000 = 4000,
    Steps5000 = 5000,
    Steps8000 = 8000,
    Steps10000 = 10000,
    Steps20000 = 20000,
    Steps25000 = 25000,
}

impl StepsPerRevolution {
    pub const fn steps_per_revolution(self) -> u16 {
        self as u16
    }

    // pub const fn steps_for_degrees(self, degrees: f32) -> f32 {
    //     degrees * self as f32 / 360.0
    // }
    //
    // pub const fn degrees_per_step(self) -> f32 {
    //     360.0 / self.value() as f32
    // }
}

/// A structure representing a stepper motor and its operational properties.
///
/// This struct abstracts the control of a stepper motor, providing properties to manage its
/// pins, steps per revolution, delays between steps, and its current state (e.g., direction
/// and enabled status).
///
/// # Type Parameters
/// - `P`: The type of the GPIO pins controlling the motor's movement (pulse, direction, and enable pins).
/// - `D`: The type representing the delay mechanism (e.g., timer or delay function).
///
/// # Fields
/// - `pulse_pin: P`
///   The GPIO pin responsible for sending the pulse signals that control the motor's steps.
///
/// - `direction_pin: P`
///   The GPIO pin responsible for setting the direction of the motor's rotation.
///
/// - `enable_pin: P`
///   The GPIO pin used to enable or disable the motor.
///
/// - `steps_per_revolution: StepsPerRevolution`
///   The number of steps required for the motor to complete one full revolution. This is specific
///   to the motor's hardware specifications.
///
/// - `delay: D`
///   The delay mechanism used to specify the interval between pulses when moving the motor. This
///   field determines the speed of the motor rotation.
///
/// - `_direction: Direction`
///   Tracks the current direction of the motor's rotation internally. The available directions
///   can be `Clockwise` or `CounterClockwise`.
///
/// - `_enabled: bool`
///   Represents whether the motor is currently enabled (`true`) or disabled (`false`).
///
///
/// # Note
/// This struct assumes the user will manage the initialization and configuration
/// of the GPIO pins and delay mechanism externally before using this structure.
pub struct StepperMotor<P, D> {
    pulse_pin: P,
    direction_pin: P,
    enable_pin: P,
    steps_per_revolution: StepsPerRevolution,
    delay: D,
    _direction: Direction,
    _enabled: bool,
}

impl<P, D> StepperMotor<P, D>
where
    P: embedded_hal::digital::OutputPin,
    D: embedded_hal_async::delay::DelayNs,
{
    const STEP_PULSE_WIDTH_US: u64 = 625 / 2;

    pub const fn step_delay(&self) -> u64 {
        Self::STEP_PULSE_WIDTH_US
    }

    pub const fn new(
        pulse_pin: P,
        direction_pin: P,
        enable_pin: P,
        delay: D,
        steps_per_revolution: StepsPerRevolution,
    ) -> Self {
        Self {
            pulse_pin,
            direction_pin,
            enable_pin,
            steps_per_revolution,
            delay,
            _direction: Clockwise,
            _enabled: false,
        }
    }

    pub fn enable_driver(&mut self) -> Result<(), P::Error> {
        // todo: confirm if this is correct in the hardware. the output should
        // be configured as a push-pull, through a voltage level converter from esp 3.3v to 5V
        // so this might be inverted.
        // when the pin is HIGH, driver enabled,
        // when the pin is LOW, driver disabled
        self.enable_pin.set_high()?;
        self._enabled = true;
        Ok(())
    }

    pub fn disable_driver(&mut self) -> Result<(), P::Error> {
        self.enable_pin.set_low()?;
        self._enabled = false;
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self._enabled
    }

    pub async fn swap_direction(&mut self) -> Result<(), P::Error> {
        self.set_direction(match self._direction {
            Clockwise => CounterClockwise,
            CounterClockwise => Clockwise,
        })
        .await
    }

    pub fn steps_per_rev(&self) -> StepsPerRevolution {
        self.steps_per_revolution
    }

    pub async fn set_direction(&mut self, direction: Direction) -> Result<(), P::Error> {
        if direction == self._direction {
            // already set
            return Ok(());
        }
        match direction {
            // todo: test these, but they are kind of wiring dependent
            Clockwise => {
                self.direction_pin.set_low()?;
            }
            CounterClockwise => {
                self.direction_pin.set_high()?;
            }
        }
        self._direction = direction;
        Timer::after_micros(2).await;

        Ok(())
    }

    pub async fn step(&mut self) -> Result<(), P::Error> {
        // stepping when disabled does nothing
        if !self.is_enabled() {
            return Ok(());
        }

        self.pulse_pin.set_high()?;
        self.delay.delay_us(Self::STEP_PULSE_WIDTH_US as u32).await;
        self.pulse_pin.set_low()?;
        Ok(())
    }
}

pub struct StepperController<P, D> {
    stepper_motor: StepperMotor<P, D>,
    angle_setpoint: f32,
    angle_estimate: f32,
    direction: Direction,
    min_step_delay_us: u32,
    max_step_delay_us: u32,
    current_step_delay_us: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_switch_mapping_steps_per_rev() {
        let switch_config = DipSwitches {
            switch1: false,
            switch2: false,
            switch3: false,
            switch4: false,
            switch5: false,
            switch6: false,
            switch7: false,
            switch8: false,
        };
    }
}
