import type { TelemetryRecord } from './telemetry';

export const MAX_HISTORY_RECORDS = 7_200;
export const GRID_HISTORY_ROWS = 11;

export function appendRecord(history: TelemetryRecord[], record: TelemetryRecord): TelemetryRecord[] {
  const next = [...history, record];
  return next.length > MAX_HISTORY_RECORDS ? next.slice(next.length - MAX_HISTORY_RECORDS) : next;
}

/** A decreased sequence/timestamp means the device has restarted. */
export function isDeviceRestart(previous: TelemetryRecord | undefined, next: TelemetryRecord): boolean {
  return previous !== undefined && (next.seq <= previous.seq || next.timestamp_ms < previous.timestamp_ms);
}

export function gridRows(history: TelemetryRecord[]): boolean[][] {
  const recent = history.slice(-GRID_HISTORY_ROWS).map((record) => record.detections);
  const blankRows = Array.from(
    { length: GRID_HISTORY_ROWS - recent.length },
    () => Array.from({ length: 10 }, () => false),
  );
  return [...blankRows, ...recent];
}
