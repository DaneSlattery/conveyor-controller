interface SerialPort {
  readable: ReadableStream<BufferSource> | null;
  open(options: { baudRate: number }): Promise<void>;
  close(): Promise<void>;
}

interface Navigator {
  serial?: {
    requestPort(): Promise<SerialPort>;
  };
}
