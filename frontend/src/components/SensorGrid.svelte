<script lang="ts">
  import type { TelemetryRecord } from '../lib/telemetry';
  export let rows: boolean[][];
  export let record: TelemetryRecord | undefined;
</script>

<article class="grid-panel">
  <div class="panel-heading"><div><span class="panel-kicker">DIGITAL INPUT ARRAY</span><h2>Conveyor edge sensors</h2><p>Live state and the last 11 acquired frames</p></div><span class="badge">GPIO[0:9]</span></div>
  <div class="conveyor-strip" aria-label="Current conveyor sensor state">
    <span class="belt-label">BELT</span>
    <div class="belt">
      {#each Array(10) as _, index}
        <i class:detected={record?.detections[index]} title={`Sensor ${index + 1}`}></i>
      {/each}
    </div>
  </div>
  <p class="history-label">FRAME HISTORY <span>oldest → newest</span></p>
  <div class="sensor-grid" aria-label="Last 11 sensor readings">
    {#each rows as row, rowIndex}
      {#each row as detected, sensorIndex}
        <div class:detected aria-label={`Sample ${rowIndex + 1}, sensor ${sensorIndex + 1}: ${detected ? 'detected' : 'clear'}`}></div>
      {/each}
    {/each}
  </div>
  <div class="grid-legend"><span><i class="active"></i>Detected</span><span><i></i>Clear</span></div>
</article>
