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
  <header class="masthead">
    <div>
      <p class="eyebrow">CONVEYOR CONTROLLER / DIAGNOSTICS</p>
      <h1>Live telemetry</h1>
      <p class="subtitle">Sensor alignment and output-shaft instructions over time.</p>
    </div>
    <SerialConnection {baudRate} {connected} status={status} detail={statusDetail} onBaudRateChange={(value) => baudRate = value} onConnect={connect} onDisconnect={disconnect} />
  </header>

  <MetricCards record={latest} />
  <section class="dashboard">
    <SensorGrid {rows} />
    <TelemetryChart {history} kind="score" />
    <TelemetryChart {history} kind="motion" />
  </section>
  <footer><span>{history.length.toLocaleString()} records retained <small>MAX 7,200</small></span><span>{malformedLines} non-telemetry line{malformedLines === 1 ? '' : 's'} ignored</span></footer>
</main>
