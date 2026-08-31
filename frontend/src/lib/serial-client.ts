import { LineBuffer, parseTelemetryLine, type TelemetryRecord } from './telemetry';

export interface SerialClientCallbacks {
  onRecord: (record: TelemetryRecord) => void;
  onMalformedLine: () => void;
  onError: (message: string) => void;
  onClosed: () => void;
}

/** Owns one read-only Web Serial session and delivers validated telemetry records. */
export class SerialTelemetryClient {
  private port: SerialPort | undefined;
  private reader: ReadableStreamDefaultReader<string> | undefined;
  private readTask: Promise<void> | undefined;
  private active = false;

  async connect(baudRate: number, callbacks: SerialClientCallbacks): Promise<void> {
    if (!navigator.serial) throw new Error('Web Serial is available in Chrome and other Chromium browsers on localhost or HTTPS.');
    this.port = await navigator.serial.requestPort();
    await this.port.open({ baudRate });
    this.active = true;
    this.readTask = this.read(this.port, callbacks);
  }

  async disconnect(): Promise<void> {
    this.active = false;
    await this.reader?.cancel().catch(() => undefined);

    // SerialPort.close() rejects while its readable stream is locked. The read
    // task releases that lock only after the TextDecoder pipe has settled.
    const readTask = this.readTask;
    await readTask?.catch(() => undefined);
    if (this.readTask === readTask) this.readTask = undefined;

    const port = this.port;
    this.port = undefined;
    await port?.close().catch(() => undefined);
  }

  private async read(port: SerialPort, callbacks: SerialClientCallbacks): Promise<void> {
    if (!port.readable) return;
    const decoder = new TextDecoderStream();
    const pipe = port.readable.pipeTo(decoder.writable);
    const reader = decoder.readable.getReader();
    this.reader = reader;
    const lines = new LineBuffer();
    try {
      while (this.active && this.reader === reader) {
        const { value, done } = await reader.read();
        if (done) break;
        for (const line of lines.push(value)) {
          try { callbacks.onRecord(parseTelemetryLine(line)); } catch { callbacks.onMalformedLine(); }
        }
      }
    } catch (error) {
      if (this.active) callbacks.onError(error instanceof Error ? error.message : 'The serial stream stopped unexpectedly.');
    } finally {
      if (this.reader === reader) this.reader = undefined;
      reader.releaseLock();
      await pipe.catch(() => undefined);

      if (this.active) {
        this.active = false;
        if (this.port === port) {
          this.port = undefined;
          await port.close().catch(() => undefined);
        }
        callbacks.onClosed();
      }
    }
  }
}
