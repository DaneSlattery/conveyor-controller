# Conveyor telemetry dashboard

Run the dashboard with `npm install` followed by `npm run dev`. Open the localhost URL in Chrome or another Chromium browser, then use **Connect serial port**. Web Serial requires a secure context (localhost is permitted).

The dashboard is intentionally read-only and expects a dedicated UTF-8 JSON-lines UART stream at 115200 baud. It ignores malformed or unrelated console lines, although a port containing only telemetry is preferable.

## JSON-lines protocol (v1)

Print one complete JSON object followed by `\n` per sensor/control sample. Use normal text serial output such as `esp_println::println!`; raw `defmt` frames are binary/encoded and require a separate decoder before a browser can consume them.

```json
{"type":"conveyor.telemetry","version":1,"seq":427,"timestamp_ms":213500,"detections":[false,false,false,false,true,true,true,true,true,false],"score":{"raw":1,"filtered":1},"motion":{"mode":"automatic","command":"move_to","target_steps":71111,"target_output_deg":1.0,"velocity_sps":null}}
```

`seq` and `timestamp_ms` are non-negative, monotonically increasing values; the latter is milliseconds since boot. A decrease resets the browser’s current session history. `detections` must contain the ten sensor booleans in firmware-array order. The dashboard retains the latest 7,200 records (one hour at 500 ms per sample).

`motion.mode` is `homing`, `automatic`, or `jogging`; `motion.command` is `stop`, `move_to`, `move_at`, or `zero_controller`.

| Command                    | `target_steps` and `target_output_deg` | `velocity_sps` |
|----------------------------|----------------------------------------|----------------|
| `move_to`                  | numbers                                | `null`         |
| `move_at`                  | `null`                                 | signed number  |
| `stop` / `zero_controller` | `null`                                 | `null`         |

Publish the latest motion command with every record, even when the command did not change. `score.filtered` may be `null` until the firmware filter produces a value.
