<script lang="ts">
  import { onDestroy } from 'svelte';
  import MetricCards from './components/MetricCards.svelte';
  import SensorGrid from './components/SensorGrid.svelte';
  import SerialConnection from './components/SerialConnection.svelte';
  import TelemetryChart from './components/TelemetryChart.svelte';
  import { appendRecord, gridRows, isDeviceRestart } from './lib/history';
  import { SerialTelemetryClient } from './lib/serial-client';
  import type { TelemetryRecord } from './lib/telemetry';

  const client = new SerialTelemetryClient();
  let baudRate = 115200;
  let status = 'Disconnected';
  let statusDetail = 'Choose the board serial port to start receiving telemetry.';
  let connected = false;
  let malformedLines = 0;
  let history: TelemetryRecord[] = [];

  $: latest = history.at(-1);
  $: rows = gridRows(history);

  onDestroy(() => {
    void client.disconnect();
  });

  async function connect() {
    try {
      status = 'Choose a serial port';
      statusDetail = 'Select the UART device in the browser dialog.';
      await client.connect(baudRate, {
        onRecord(record) { history = isDeviceRestart(latest, record) ? [record] : appendRecord(history, record); },
        onMalformedLine() { malformedLines += 1; },
        onError(message) { status = 'Serial read failed'; statusDetail = message; },
        onClosed() {
          connected = false;
          if (status === 'Connected') {
            status = 'Disconnected';
            statusDetail = 'The serial stream ended. Received history remains visible.';
          }
        },
      });
      history = [];
      malformedLines = 0;
      connected = true;
      status = 'Connected';
      statusDetail = `Reading JSON-lines telemetry at ${baudRate} baud.`;
    } catch (error) {
      connected = false;
      status = 'Connection failed';
      statusDetail = error instanceof Error ? error.message : 'The serial port could not be opened.';
      await client.disconnect();
    }
  }

  async function disconnect() {
    await client.disconnect();
    connected = false;
    if (status === 'Connected' || status === 'Choose a serial port') {
      status = 'Disconnected';
      statusDetail = 'The received history remains visible until the next connection.';
    }
  }
</script>

<svelte:head><meta name="description" content="Real-time conveyor sensor telemetry dashboard" /></svelte:head>

<main>
  <header class="application-bar">
    <div class="brand"><span class="brand-mark">▰</span><span>Conveyor alignment controller</span><small>embedded telemetry</small></div>
    <div class="run-controls"><span class:active={connected} class="run-indicator"></span><span>{connected ? 'RUNNING' : 'STOPPED'}</span></div>
  </header>

  <section class="toolstrip" aria-label="Connection controls">
    <span class="tool-label">LIVE DEVICE LINK</span>
    <SerialConnection {baudRate} {connected} status={status} detail={statusDetail} onBaudRateChange={(value) => baudRate = value} onConnect={connect} onDisconnect={disconnect} />
  </section>

  <section class="model-title">
    <div><span class="crumb">CONVEYOR CONTROL / ALIGNMENT LOOP</span><h1>Controller monitor</h1></div>
    <p>Measurement → filter → stepper command</p>
  </section>

  <section class="model-canvas" aria-label="Conveyor controller telemetry">
    <MetricCards record={latest} {history} />
    <section class="dashboard">
      <SensorGrid {rows} record={latest} />
      <TelemetryChart {history} kind="score" />
      <TelemetryChart {history} kind="motion" />
    </section>
  </section>
  <footer><span>{history.length.toLocaleString()} samples in workspace <small>BUFFER 7,200</small></span><span>{malformedLines} non-telemetry line{malformedLines === 1 ? '' : 's'} ignored</span></footer>
</main>
