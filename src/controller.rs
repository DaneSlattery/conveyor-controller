//! a PID implementation

use embassy_time::{Duration, Instant};

pub struct PidController {
    kp: f32,
    ki: f32,
    kd: f32,
    setpoint: Option<f32>,
    err_sum: f32,
    last_err: f32,
    last_time: Instant,
    sample_time: Duration,
}

impl PidController {
    pub fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint: None,
            err_sum: 0.0,
            last_err: 0.0,
            last_time: Instant::from_millis(0),
            sample_time: Duration::from_millis(1000),
        }
    }

    pub fn with_setpoint(mut self: Self, setpoint: f32) -> Self {
        self.setpoint = Some(setpoint);
        return self;
    }

    pub fn set_setpoint(&mut self, setpoint: f32) {
        self.setpoint = Some(setpoint);
    }

    pub fn set_tunings(&mut self, kp: f32, ki: f32, kd: f32) {
        let sample_time_sec: f32 = self.sample_time.as_secs() as f32;
        self.kp = kp;
        self.ki = ki * sample_time_sec;
        self.kd = kd / sample_time_sec;
    }

    pub fn set_sample_time(&mut self, time: Duration) {
        if time.le(&Duration::from_secs(0)) {
            return;
        }
        let ratio = (time.as_millis() / self.sample_time.as_millis()) as f32;
        self.ki *= ratio;
        self.kd /= ratio;
        self.sample_time = time;
    }

    pub fn compute(&mut self, input: f32) -> Option<f32> {
        if self.setpoint.is_none() {
            return None;
        }
        let now = Instant::now();
        let time_change = now - self.last_time;

        if time_change < self.sample_time {
            return None;
        }
        let setpoint = self.setpoint.unwrap();
        let error = setpoint - input;
        self.err_sum += error;
        let d_err = error - self.last_err;

        let output = self.kp * error + self.ki * self.err_sum + self.kd * d_err;

        self.last_time = now;
        self.last_err = error;
        return Some(output);
    }
}
