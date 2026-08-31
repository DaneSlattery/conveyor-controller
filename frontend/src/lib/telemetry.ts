export const SENSOR_COUNT = 10;

export type MotionMode = 'homing' | 'automatic' | 'jogging';
export type MotionCommand = 'stop' | 'move_to' | 'move_at' | 'zero_controller';

export interface TelemetryRecord {
  type: 'conveyor.telemetry';
  version: 1;
  seq: number;
  timestamp_ms: number;
  detections: boolean[];
  score: {
    raw: number;
    filtered: number | null;
  };
  motion: {
    mode: MotionMode;
    command: MotionCommand;
    target_steps: number | null;
    target_output_deg: number | null;
    velocity_sps: number | null;
  };
}

const modes = new Set<MotionMode>(['homing', 'automatic', 'jogging']);
const commands = new Set<MotionCommand>(['stop', 'move_to', 'move_at', 'zero_controller']);

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function nullableNumber(value: unknown): value is number | null {
  return value === null || isFiniteNumber(value);
}

/** Parse and validate one newline-delimited telemetry record. */
export function parseTelemetryLine(line: string): TelemetryRecord {
  const parsed: unknown = JSON.parse(line);
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new Error('Telemetry line must be a JSON object.');
  }

  const value = parsed as Record<string, unknown>;
  const score = value.score as Record<string, unknown> | null;
  const motion = value.motion as Record<string, unknown> | null;

  if (value.type !== 'conveyor.telemetry' || value.version !== 1) {
    throw new Error('Unsupported telemetry type or version.');
  }
  if (!Number.isSafeInteger(value.seq) || (value.seq as number) < 0) {
    throw new Error('Telemetry seq must be a non-negative integer.');
  }
  if (!Number.isSafeInteger(value.timestamp_ms) || (value.timestamp_ms as number) < 0) {
    throw new Error('Telemetry timestamp_ms must be a non-negative integer.');
  }
  if (!Array.isArray(value.detections) || value.detections.length !== SENSOR_COUNT || !value.detections.every((item) => typeof item === 'boolean')) {
    throw new Error(`Telemetry detections must contain exactly ${SENSOR_COUNT} booleans.`);
  }
  if (!score || !isFiniteNumber(score.raw) || !nullableNumber(score.filtered)) {
    throw new Error('Telemetry score is invalid.');
  }
  if (!motion || !modes.has(motion.mode as MotionMode) || !commands.has(motion.command as MotionCommand)) {
    throw new Error('Telemetry motion mode or command is invalid.');
  }
  if (!nullableNumber(motion.target_steps) || !nullableNumber(motion.target_output_deg) || !nullableNumber(motion.velocity_sps)) {
    throw new Error('Telemetry motion values must be numbers or null.');
  }

  validateCommandValues(motion);
  return parsed as TelemetryRecord;
}

function validateCommandValues(motion: Record<string, unknown>): void {
  const hasTarget = motion.target_steps !== null && motion.target_output_deg !== null;
  const hasVelocity = motion.velocity_sps !== null;

  if (motion.command === 'move_to' && (!hasTarget || hasVelocity)) {
    throw new Error('move_to requires targets and no velocity.');
  }
  if (motion.command === 'move_at' && (hasTarget || !hasVelocity)) {
    throw new Error('move_at requires velocity and no targets.');
  }
  if ((motion.command === 'stop' || motion.command === 'zero_controller') && (hasTarget || hasVelocity)) {
    throw new Error(`${motion.command} must not include target or velocity values.`);
  }
}

/** Splits decoded chunks into complete lines while retaining a partial tail. */
export class LineBuffer {
  private tail = '';

  push(chunk: string): string[] {
    const lines = (this.tail + chunk).split(/\r?\n/);
    this.tail = lines.pop() ?? '';
    return lines.filter((line) => line.trim().length > 0);
  }

  reset(): void {
    this.tail = '';
  }
}
