<script lang="ts">
  export let rows: boolean[][];
</script>

<article class="grid-panel">
  <div class="panel-heading"><div><h2>Sensor history</h2><p>Oldest at top · newest at bottom</p></div><span class="badge">11 samples × 10 sensors</span></div>
  <div class="sensor-grid" aria-label="Last 11 sensor readings">
    {#each rows as row, rowIndex}
      {#each row as detected, sensorIndex}
        <div class:detected aria-label={`Sample ${rowIndex + 1}, sensor ${sensorIndex + 1}: ${detected ? 'detected' : 'clear'}`}></div>
      {/each}
    {/each}
  </div>
  <div class="grid-legend"><span><i class="active"></i>Detected</span><span><i></i>Clear</span></div>
</article>
