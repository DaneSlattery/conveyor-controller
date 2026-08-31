<script lang="ts">
  import type { TelemetryRecord } from '../lib/telemetry';
  export let record: TelemetryRecord | undefined;
  const score = (value: number | null | undefined) => value === null || value === undefined ? '—' : value.toFixed(0);
  const command = (value: string | undefined) => value?.replace('_', ' ') ?? '—';
</script>

<section class="metrics" aria-label="Current telemetry values">
  <article><span>Raw score</span><strong>{score(record?.score.raw)}</strong></article>
  <article><span>Filtered score</span><strong>{score(record?.score.filtered)}</strong></article>
  <article><span>Control mode</span><strong class="text-value">{record?.motion.mode ?? '—'}</strong></article>
  <article><span>Instruction</span><strong class="text-value">{command(record?.motion.command)}</strong></article>
</section>
