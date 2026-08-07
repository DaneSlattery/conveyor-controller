• The strongest direction is a “functional core, imperative shell”:

- GPIO adapters read sensors.
- Pure logic converts a [bool; N] snapshot into a score.
- A runtime Config controls polarity, orientation, calibration, and
  timing.

- Firmware owns Embassy, logging, pin assignment, and error policy.

That will remove most mocking and test duplication while keeping the
embedded path allocation-free.

## Highest-impact findings

1. The portable library currently imports firmware-only crates

src/lib.rs:16 imports embassy_time, esp_println, and log, but those
dependencies exist only for the ESP target. Consequently, host tests
do not compile:

unresolved import `embassy_time`
unresolved import `esp_println`
unresolved import `log`

Those imports are unused except for logging and should be removed from
the library. Logging the samples belongs in the firmware task.

heapless::Vec is also unused and can currently be removed.

2. Sensor errors become panics

src/lib.rs:38 calls unwrap() on GPIO operations:

pub fn is_triggered(&mut self) -> bool {
if self.active_low {
return self.pin.is_low().unwrap();
}
self.pin.is_high().unwrap()
}

InputPin explicitly models fallible reads. Propagate that error:

pub fn is_triggered(&mut self) -> Result<bool, P::Error> {
let high = self.pin.is_high()?;
Ok(if self.active_low { !high } else { high })
}

This also makes failure behavior testable. Firmware can then decide
whether to retry, retain the last measurement, log the error, or enter
a safe state.

3. The scoring behavior and documentation disagree

src/lib.rs:65 only counts active sensors:

score = active_count - N / 2;

Therefore, sensor order does not matter. These patterns produce
identical scores:

[X, 0, 0, 0]
[0, 0, 0, X]

But the documentation and tests discuss left-to-right position as if
ordering changes the result. Decide which physical model is correct:

- If the sensors measure how much belt overlaps an edge array, active
  count is probably correct.

- If individual positions represent belt location, use positional
  weights rather than active count.

The current right-side tests also appear inconsistent. For
array_on_left == false, one active sensor gives 1 - 2 == -1, but src/
lib.rs:256. Once compilation is fixed, this test should fail.

4. The firmware creates four sensors but uses two

src/bin/main.rs:67 constructs four sensors, while src/bin/main.rs:84:

let sensor_array =
ConveyorSensorArray::new([conveyor_sensor_1, conveyor_sensor_2],
true);

Sensors 3 and 4 are unused. This may simply be unfinished code, but it
also makes the configured midpoint and resulting score different from
what the tests describe.

5. The Embassy spawn call likely unwraps the wrong value

src/bin/main.rs:89 currently has:

spawner.spawn(measure_array(sensor_array).unwrap());

The usual shape is:

spawner.spawn(measure_array(sensor_array)).unwrap();

The task function generates a spawn token; spawn returns the Result.

6. build.rs has an argument bounds bug

build.rs:15 checks:

if args.len() > 1 {
let kind = &args[1];
let what = &args[2];
}

Accessing args[2] requires args.len() > 2. Prefer slice matching:

if let [_, kind, what, ..] = args.as_slice() {
// ...
}

## Suggested core design

Make polarity and orientation named runtime values instead of
booleans:

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Polarity {
ActiveHigh,
ActiveLow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArraySide {
Left,
Right,
}

#[derive(Clone, Copy, Debug)]
pub struct ScoringConfig {
pub side: ArraySide,
pub center: i16,
}

Named enums prevent calls such as this:

ConveyorSensorArray::new(sensors, true)

where the reader must remember what true means.

Instead, the call becomes:

ConveyorSensorArray::new(
sensors,
ScoringConfig {
side: ArraySide::Left,
center: 2,
},
)

A runtime center is also more flexible than assuming N / 2. It
supports odd arrays, partial sensor arrays, asymmetric calibration,
and real-world mechanical offsets.

## Separate acquisition from calculation

Right now detection_score() performs three jobs:

1. Reads hardware.
2. Calculates a score.
3. Logs the readings.

Split those:

pub fn score<const N: usize>(
detections: &[bool; N],
config: ScoringConfig,
) -> i16 {
let active = detections.iter().filter(|&&value| value).count() as
i16;
let raw = active - config.center;

      match config.side {
          ArraySide::Left => -raw,
          ArraySide::Right => raw,
      }
}

Then hardware acquisition can be separate:

impl<P, const N: usize> ConveyorSensorArray<P, N>
where
P: InputPin,
{
pub fn sample(&mut self) -> Result<[bool; N], P::Error> {
let mut result = [false; N];

          for (result, sensor) in result.iter_mut().zip(&mut
          self.sensors) {
              *result = sensor.is_triggered()?;
          }

          Ok(result)
      }
}

Firmware becomes explicit:

match sensors.sample() {
Ok(detections) => {
let score = score(&detections, config);
info!("detections={detections:?}, score={score}");
}
Err(error) => {
// Apply the chosen operational policy.
}
}

Benefits:

- Scoring tests require no GPIO mocks.
- GPIO behavior needs only a couple of adapter tests.
- Logging is not coupled to computation.
- Recorded production samples can be replayed in host tests.
- Alternative scoring algorithms become simple functions.

## Runtime-friendly configuration

Keep hardware topology compile-time where Rust and the HAL benefit
from it:

- Pin ownership
- Maximum sensor count
- GPIO concrete types
- Task allocation

Make operational behavior runtime-configurable:

pub struct ControllerConfig {
pub scoring: ScoringConfig,
pub sample_period: Duration,
pub deadband: i16,
pub maximum_correction: i16,
}

You may want sample_period_ms: u32 in the portable library instead of
embassy_time::Duration. Convert it to Duration at the firmware
boundary. That keeps the core independent of Embassy.

A useful distinction is:

Hardware configuration       Runtime controller configuration
  ----------------------       --------------------------------
GPIO23, GPIO22               sample interval
number of physical pins      left/right orientation
pull-up mode                 polarity
calibrated center
deadband
gain or correction limit

Trying to make pin ownership fully runtime-configurable usually fights
the HAL’s static safety model. Calibration and controller behavior are
much better runtime configuration candidates.

## Reducing the test duplication

The ten tests are mostly the same setup repeated. Once scoring is
pure, replace them with a table:

#[test]
fn scores_sensor_patterns() {
let cases = [
([false, false, false, false], ArraySide::Left,  2),
([false, false, false, true ], ArraySide::Left,  1),
([false, false, true,  true ], ArraySide::Left,  0),
([false, true,  true,  true ], ArraySide::Left, -1),
([true,  true,  true,  true ], ArraySide::Left, -2),

          ([false, false, false, false], ArraySide::Right, -2),
          ([true,  false, false, false], ArraySide::Right, -1),
          ([true,  true,  false, false], ArraySide::Right,  0),
          ([true,  true,  true,  false], ArraySide::Right,  1),
          ([true,  true,  true,  true ], ArraySide::Right,  2),
      ];

      for (detections, side, expected) in cases {
          let config = ScoringConfig { side, center: 2 };
          assert_eq!(score(&detections, config), expected);
      }
}

Then retain small focused mock tests:

- Active-high translates High to true.
- Active-low translates Low to true.
- GPIO errors are returned rather than panicking.

This gives better coverage with much less code.

## Interesting patterns worth considering

A Detection newtype can keep raw integers from spreading:

pub struct DetectionScore(i16);

Later, it can enforce limits or expose semantic operations such as
direction() and magnitude().

A strategy trait is worthwhile only if you genuinely expect multiple
algorithms:

pub trait ScoringStrategy {
fn score(&self, detections: &[bool]) -> DetectionScore;
}

Possible implementations might include:

- ActiveCount
- PositionWeighted
- CalibratedWeights
- Debounced<S>

I would begin with a plain pure function. Introduce the trait when a
second algorithm actually exists.

For noise, a small stateful decorator is interesting:

GPIO sample
→ debounce/filter
→ score
→ deadband
→ controller output

Each stage can remain independently testable. Avoid building a generic
processing framework prematurely; small structs with update(...)
methods are enough.

## Smaller cleanup points

- Replace the active_low: bool constructor default with an explicit
  Polarity argument or active_low(pin) constructor.

- Use for (slot, sensor) in detections.iter_mut().zip(&mut
  self.sensors) rather than indexing.

- The else assigning false is unnecessary because the array starts
  false; direct assignment is clearer.

- Omit explicit return for the final expression.
- Remove unused imports pin and TransactionKind.
- NUM_SENSORS is declared repeatedly but unused.
- create_conveyor_sensor_array takes four references and manually
  clones each. Prefer accepting [Mock; N], or eliminate it once
  scoring becomes pure.

- Replace array_on_left with ArraySide; boolean configuration tends to
  become confusing.

- Decide and document behavior for N == 0, odd N, and arrays too large
  for i16.

- Consider i32 for intermediate calculations, then return a checked or
  clamped i16.

I could not execute the tests because the portable library currently
references target-only firmware crates. The first refactor should be
removing those imports and moving the detection logging into
measure_array; after that, the tests will reveal the scoring
expectation mismatch.