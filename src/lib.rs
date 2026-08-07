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

use embassy_time::{Duration, Ticker};
use esp_println::println;
use heapless::Vec;
use log::info;

/// Represents a single conveyor belt sensor
pub struct ConveyorSensor<P> {
    pin: P,
    active_low: bool,
}

impl<P> ConveyorSensor<P>
where
    P: embedded_hal::digital::InputPin,
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            active_low: false,
        }
    }

    pub fn is_triggered(&mut self) -> bool {
        if self.active_low {
            return self.pin.is_low().unwrap();
        }
        self.pin.is_high().unwrap()
    }
}

pub struct ConveyorSensorArray<P, const N: usize> {
    sensors: [ConveyorSensor<P>; N], // the order matters, let's say sensors are arranged left to right
    // todo: could use a inter-sensor gap to calculate real spatial co-ordinates/quantisation
    /// True when the sensor array is on the left side of the belt, false on the right.
    array_on_left: bool,
}

impl<P, const N: usize> ConveyorSensorArray<P, N>
where
    P: embedded_hal::digital::InputPin,
{
    pub fn new(sensors: [ConveyorSensor<P>; N], array_on_left: bool) -> Self {
        Self {
            sensors,
            array_on_left,
        }
    }

    /// Calculates the current
    pub fn detection_score(&mut self) -> i16 // note i16 might not be big enough based on N
    {
        // let's say we have 4 sensors [1 ,2, 3, 4]
        // if sensor 1,2 are active, the score is zero
        // if sensor 1 only is active, the score is -1,
        // if no sensor is active, the score is -2
        // if sensor 1,2,3 are active, the score is 1
        // etc
        // we assume if the sensor array is on the left of the conveyor, that lower ranked sensors indicate leftwards movement,
        // and higher ranked sensors indicate rightwards movement


        let mut detections: [bool;N] = [false;N];
        // we might want to weight sensors further from zero higher
        let halfway = (N / 2) as i16; // need to consider odd number of sensors
        let mut score: i16 = 0;
        for i in 0..N {
            let sensor = &mut self.sensors[i];

            if sensor.is_triggered() {
                score += 1;
                detections[i] = true;
            }
            else{
                
            detections[i] = false;
            }
        }
        score -= halfway;

        // if the sensor array is on the left, detections are counted from right to left
        //  so the score is proportional to more "leftwards" movement of the belt
        //
        // if the sensor array is on the right, detections are counted from left to right
        //  so the score is proportional to more "rightwards" movement of the belt
        //  our unit system says left is "negative"

        if self.array_on_left {
            score *= -1;
        }
        info!("{detections:?}");

        return score;
    }
}








#[cfg(test)]
mod tests {
    use super::*;
    use core::pin::pin;
    use embedded_hal_mock::eh1::digital::{State, Transaction};

    use embedded_hal_mock::eh1::digital::{Mock as PinMock, Mock, TransactionKind};

    #[test]
    fn detection_score_all_left_left_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_missing_conveyor();
        let mut pin3 = create_missing_conveyor();
        let mut pin4 = create_missing_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,true);

        let score = sensors.detection_score();
        // [ 0, 0, 0, 0] -> 2
        assert_eq!(score, 2);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_left_left_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_missing_conveyor();
        let mut pin3 = create_missing_conveyor();
        let mut pin4 = create_triggered_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,true);

        let score = sensors.detection_score();
        // [ 0, 0, 0, X] -> 1
        assert_eq!(score, 1);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }



    #[test]
    fn detection_score_middle_left_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_missing_conveyor();
        let mut pin3 = create_triggered_conveyor();
        let mut pin4 = create_triggered_conveyor();

        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,true);

        let score = sensors.detection_score();
        // [ 0, 0, X, X] -> 0

        assert_eq!(score, 0);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_right_left_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_triggered_conveyor();
        let mut pin3 = create_triggered_conveyor();
        let mut pin4 = create_triggered_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,true);

        let score = sensors.detection_score();
        // [ 0, X, X, X] -> -1
        assert_eq!(score, -1);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_all_right_left_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_triggered_conveyor();
        let mut pin2 = create_triggered_conveyor();
        let mut pin3 = create_triggered_conveyor();
        let mut pin4 = create_triggered_conveyor();


        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,true);


        let score = sensors.detection_score();
        // [ X, X, X, X] -> -2
        assert_eq!(score, -2);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }


    // RIGHT


    #[test]
    fn detection_score_all_left_right_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_missing_conveyor();
        let mut pin3 = create_missing_conveyor();
        let mut pin4 = create_missing_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,false);

        let score = sensors.detection_score();
        // [ 0, 0, 0, 0] -> 2
        assert_eq!(score, -2);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_left_right_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_triggered_conveyor();
        let mut pin2 = create_missing_conveyor();
        let mut pin3 = create_missing_conveyor();
        let mut pin4 = create_missing_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,false);

        let score = sensors.detection_score();
        // [ X, 0, 0, 0] -> -1
        assert_eq!(score, 1);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }



    #[test]
    fn detection_score_middle_right_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_triggered_conveyor();
        let mut pin2 = create_triggered_conveyor();
        let mut pin3 = create_missing_conveyor();
        let mut pin4 = create_missing_conveyor();

        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,false);

        let score = sensors.detection_score();
        // [ 0, 0, X, X] -> 0

        assert_eq!(score, 0);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_right_right_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_missing_conveyor();
        let mut pin2 = create_triggered_conveyor();
        let mut pin3 = create_triggered_conveyor();
        let mut pin4 = create_triggered_conveyor();
        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,false);

        let score = sensors.detection_score();
        // [ 0, X, X, X] -> -1
        assert_eq!(score, -1);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    #[test]
    fn detection_score_all_right_right_conveyor() {
        const NUM_SENSORS: usize = 4;
        let mut pin1 = create_triggered_conveyor();
        let mut pin2 = create_triggered_conveyor();
        let mut pin3 = create_triggered_conveyor();
        let mut pin4 = create_triggered_conveyor();


        let mut sensors = create_conveyor_sensor_array(&pin1,&pin2,&pin3,&pin4,false);


        let score = sensors.detection_score();
        // [ X, X, X, X] -> -2
        assert_eq!(score, -2);
        pin1.done();
        pin2.done();
        pin3.done();
        pin4.done();
    }

    fn create_conveyor_sensor_array(pin1: &Mock,pin2:&Mock,pin3:&Mock,pin4:&Mock, left: bool) -> ConveyorSensorArray<Mock,4>
    {
        let sensor1 = ConveyorSensor::new(pin1.clone());
        let sensor2 = ConveyorSensor::new(pin2.clone());
        let sensor3 = ConveyorSensor::new(pin3.clone());
        let sensor4 = ConveyorSensor::new(pin4.clone());

        let mut sensors = ConveyorSensorArray::new([sensor1, sensor2, sensor3, sensor4],left);
        return sensors;
    }

    fn create_triggered_conveyor() -> PinMock {
        let transactions = [Transaction::get(State::High)];
        let mock = Mock::new(&transactions);
        mock
    }

    fn create_missing_conveyor() -> PinMock {
        let transactions = [Transaction::get(State::Low)];
        let mock = Mock::new(&transactions);
        mock
    }
}
