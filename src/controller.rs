//! a PID implementation

use embassy_time::{Duration, Instant};

pub struct PidController {
    kp: f32,
    ki: f32,
    kd: f32,
    setpoint: f32,
    i_term: f32,
    last_input: f32,
    out_min: f32,
    out_max: f32,
    output:f32,
    input: f32,
    last_time: Instant,
    sample_time: Duration,
    pid_mode: PidMode
}


pub enum PidMode{
    Auto,
    Manual
}
impl PidController {
    pub fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint: 0.0,
            i_term: 0.0,
            last_input: 0.0,
            out_min: 0.0,
            out_max: 0.0,
            input: 0.0,
            output: 0.0,
            last_time: Instant::from_millis(0),
            sample_time: Duration::from_millis(500),
            pid_mode: PidMode::Auto
        }
    }

    pub fn with_setpoint(mut self: Self, setpoint: f32) -> Self {
        self.setpoint = setpoint;
        self
    }

    pub fn with_out_min(mut self: Self, out_min: f32) -> Self {
        self.out_min = out_min;
        self
    }

    pub fn with_out_max(mut self: Self, out_max: f32) -> Self {
        self.out_max = out_max;
        self
    }



    pub fn set_mode(&mut self, mode: PidMode) {

        if let PidMode::Manual = self.pid_mode && matches!(mode, PidMode::Auto){
            // changing to pid mode
            self.last_input = self.input;
            self.i_term = self.output;
            if (self.i_term>self.out_max)
            {
                self.i_term=self.out_max;
            }
            else if (self.i_term<self.out_min)
            {
                self.i_term=self.out_min;
            }
        }
        self.pid_mode = mode;
    }

    pub fn set_setpoint(&mut self, setpoint: f32) {
        self.setpoint = setpoint;
    }

    pub fn set_input(&mut self, input: f32) {
        self.input = input;
    }
    pub fn get_output(&mut self) -> f32{
        self.output
    }
    pub fn set_output(&mut self, output: f32) {
        self.output = output;
    }

    pub fn set_tunings(&mut self, kp: f32, ki: f32, kd: f32) {
        let sample_time_sec: f32 = self.sample_time.as_micros() as f32/1_000_000.0;
        self.kp = kp;
        self.ki = ki * sample_time_sec;
        self.kd = kd / sample_time_sec;
    }

    pub fn set_sample_time(&mut self, time: Duration) {
        if time.le(&Duration::from_secs(0)) {
            return;
        }
        let ratio = (time.as_micros() / self.sample_time.as_micros()) as f32;
        self.ki *= ratio;
        self.kd /= ratio;
        self.sample_time = time;
    }

    pub fn compute(&mut self) {

        if let PidMode::Manual = self.pid_mode {
            return ;
        }

        let now = Instant::now();
        let time_change = now - self.last_time;

        if time_change < self.sample_time {
            return ;
        }
        let error = self.setpoint - self.input;
        self.i_term += (error*self.ki);


        if (self.i_term>self.out_max)
        {
            self.i_term=self.out_max;
        }
        else if (self.i_term<self.out_min)
        {
            self.i_term=self.out_min;
        }
        let d_input = self.input - self.last_input;

        self.output = self.kp * error + self.i_term - self.kd * d_input;
        if ( self.output>self.out_max)
        {
            self.output=self.out_max;
        }
        else if ( self.output<self.out_min)
        {
            self.output=self.out_min;
        }
        self.last_time = now;
        self.last_input = self.input;

    }
}
