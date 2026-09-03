<script lang="ts">
  import type { TelemetryRecord } from '../lib/telemetry';
  export let record: TelemetryRecord | undefined;
  export let history: TelemetryRecord[];
  const score = (value: number | null | undefined) => value === null || value === undefined ? '—' : value.toFixed(0);
  const value = (input: number | null | undefined, digits = 1) => input === null || input === undefined ? '—' : input.toFixed(digits);
  const command = (input: string | undefined) => input?.replace('_', ' ') ?? 'awaiting data';
  $: samplePeriod = history.length > 1 ? (history.at(-1)!.timestamp_ms - history[0].timestamp_ms) / (history.length - 1) : undefined;
  $: activeSensors = record?.detections.filter(Boolean).length;
  $: filterDelta = record?.score.filtered === null || record?.score.filtered === undefined ? undefined : record.score.raw - record.score.filtered;
</script>

<section class="metrics" aria-label="Control loop state">
  <article class="measurement-block">
    <span class="block-kind">MEASUREMENT</span><strong>{score(record?.score.raw)}</strong><em>raw alignment score</em>
    <small>{activeSensors === undefined ? 'Waiting for sensor frame' : `${activeSensors} / 10 sensors asserted`}</small>
  </article>
  <article class="filter-block">
    <span class="block-kind">FILTER ESTIMATE</span><strong>{score(record?.score.filtered)}</strong><em>filtered alignment score</em>
    <small>{filterDelta === undefined ? 'Filter not primed' : `raw − filtered: ${value(filterDelta, 2)}`}</small>
  </article>
  <article class="controller-block">
    <span class="block-kind">CONTROLLER STATE</span><strong class="text-value">{record?.motion.mode ?? 'idle'}</strong><em>{command(record?.motion.command)}</em>
    <small>{samplePeriod === undefined ? 'Awaiting sample cadence' : `${value(samplePeriod, 0)} ms mean sample period`}</small>
  </article>
  <article class="actuator-block">
    <span class="block-kind">STEPPER OUTPUT</span>
    {#if record?.motion.command === 'move_at'}
      <strong>{value(record.motion.velocity_sps, 0)} <small>steps/s</small></strong><em>velocity command</em>
    {:else}
      <strong>{value(record?.motion.target_output_deg, 2)}<small>°</small></strong><em>target output angle</em>
    {/if}
    <small>{record?.motion.target_steps === null || record?.motion.target_steps === undefined ? 'No absolute step target' : `${record.motion.target_steps.toLocaleString()} target steps`}</small>
  </article>
</section>
