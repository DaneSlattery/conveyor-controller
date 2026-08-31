import { describe, expect, it } from 'vitest';
import { appendRecord, gridRows, isDeviceRestart } from './history';
import { LineBuffer, parseTelemetryLine, type TelemetryRecord } from './telemetry';

const line = JSON.stringify({
  type: 'conveyor.telemetry', version: 1, seq: 4, timestamp_ms: 2000,
  detections: [false, false, false, false, false, true, true, true, true, true],
  score: { raw: 0, filtered: 0 },
  motion: { mode: 'automatic', command: 'move_to', target_steps: 0, target_output_deg: 0, velocity_sps: null },
});

describe('telemetry protocol', () => {
  it('parses a valid record', () => {
    expect(parseTelemetryLine(line).motion.command).toBe('move_to');
  });

  it('rejects invalid command fields', () => {
    expect(() => parseTelemetryLine(line.replace('"velocity_sps":null', '"velocity_sps":12'))).toThrow('move_to');
  });

  it('buffers partial serial lines', () => {
    const buffer = new LineBuffer();
    expect(buffer.push(line.slice(0, 12))).toEqual([]);
    expect(buffer.push(`${line.slice(12)}\r\n`)).toEqual([line]);
  });

  it('keeps grid rows in chronological order', () => {
    const record = parseTelemetryLine(line);
    const rows = gridRows([record]);
    expect(rows).toHaveLength(11);
    expect(rows[10]).toEqual(record.detections);
  });

  it('detects a device restart and appends non-restarted data', () => {
    const first = parseTelemetryLine(line);
    const next = { ...first, seq: 5, timestamp_ms: 2500 } as TelemetryRecord;
    expect(isDeviceRestart(first, next)).toBe(false);
    expect(isDeviceRestart(next, first)).toBe(true);
    expect(appendRecord([first], next)).toHaveLength(2);
  });
});
