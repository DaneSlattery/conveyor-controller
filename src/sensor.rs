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

    pub fn triggered(&mut self) -> Result<bool, P::Error> {
        if self.active_low {
            return self.pin.is_low();
        }
        self.pin.is_high()
    }
}

pub enum ArraySide {
    Left,
    Right,
}

pub struct ConveyorSensorArray<P, const N: usize> {
    sensors: [ConveyorSensor<P>; N], // the order matters, let's say sensors are arranged left to right
    // todo: could use a inter-sensor gap to calculate real spatial co-ordinates/quantisation
    /// True when the sensor array is on the left side of the belt, false on the right.
    array_side: ArraySide,
}

// let's say we have 4 sensors [1 ,2, 3, 4]
// if sensor 1,2 are active, the score is zero
// if sensor 1 only is active, the score is -1,
// if no sensor is active, the score is -2
// if sensor 1,2,3 are active, the score is 1
// etc
// we assume if the sensor array is on the left of the conveyor, that lower ranked sensors indicate leftwards movement,
// and higher ranked sensors indicate rightwards movement
// todo: consider cases where the detection is clearly swapped, eg if the sensor array is
// on the left, and we see [true,false,false,false], then something is wrong!
pub fn score<const N: usize>(detections: &[bool; N], array_side: &ArraySide) -> i16 {
    let active = detections.iter().filter(|&&d| d).count() as i16;
    let raw = active - ((N / 2) as i16);
    match array_side {
        ArraySide::Right => raw,
        ArraySide::Left => -raw,
    }
}

impl<P, const N: usize> ConveyorSensorArray<P, N>
where
    P: embedded_hal::digital::InputPin,
{
    pub fn new(sensors: [ConveyorSensor<P>; N], array_side: ArraySide) -> Self {
        Self {
            sensors,
            array_side,
        }
    }

    pub fn array_side(&self) -> &ArraySide {
        &self.array_side
    }

    pub fn sample(&mut self) -> Result<[bool; N], P::Error> {
        let mut detections: [bool; N] = [false; N];

        for (d, mut s) in detections.iter_mut().zip(&mut self.sensors) {
            *d = s.triggered()?;
        }
        Ok(detections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::digital::{State, Transaction};

    use embedded_hal_mock::eh1::digital::{Mock as PinMock, Mock};

    #[test]
    fn scores_sensor_patterns() {
        let cases = [
            ([false, false, false, false], ArraySide::Left, 2),
            ([false, false, false, true], ArraySide::Left, 1),
            ([false, false, true, true], ArraySide::Left, 0),
            ([false, true, true, true], ArraySide::Left, -1),
            ([true, true, true, true], ArraySide::Left, -2),
            ([false, false, false, false], ArraySide::Right, -2),
            ([true, false, false, false], ArraySide::Right, -1),
            ([true, true, false, false], ArraySide::Right, 0),
            ([true, true, true, false], ArraySide::Right, 1),
            ([true, true, true, true], ArraySide::Right, 2),
        ];

        for (detections, side, expected) in cases {
            assert_eq!(score(&detections, &side), expected);
        }
    }

    fn create_conveyor_sensor_array(
        pin1: &Mock,
        pin2: &Mock,
        pin3: &Mock,
        pin4: &Mock,
        left: bool,
    ) -> ConveyorSensorArray<Mock, 4> {
        let sensor1 = ConveyorSensor::new(pin1.clone());
        let sensor2 = ConveyorSensor::new(pin2.clone());
        let sensor3 = ConveyorSensor::new(pin3.clone());
        let sensor4 = ConveyorSensor::new(pin4.clone());

        let mut sensors =
            ConveyorSensorArray::new([sensor1, sensor2, sensor3, sensor4], ArraySide::Left);
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
