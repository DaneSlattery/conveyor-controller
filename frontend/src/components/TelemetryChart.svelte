<script lang="ts">
  import { onMount } from 'svelte';
  import Chart from 'chart.js/auto';
  import type { ChartConfiguration, ChartOptions, Point } from 'chart.js';
  import type { TelemetryRecord } from '../lib/telemetry';

  export let history: TelemetryRecord[];
  export let kind: 'score' | 'motion';
  let canvas: HTMLCanvasElement;
  let chart: Chart | undefined;

  // `history` must be passed explicitly so Svelte tracks it as a dependency.
  $: if (chart) updateChart(history);

  onMount(() => {
    chart = new Chart(canvas, kind === 'score' ? scoreConfig() : motionConfig());
    updateChart(history);
    return () => chart?.destroy();
  });

  function updateChart(records: TelemetryRecord[]) {
    if (!chart) return;
    const points = records.map((record) => ({ x: record.timestamp_ms / 1000, record }));
    if (kind === 'score') {
      chart.data.datasets[0].data = points.map(({ x, record }) => ({ x, y: record.score.raw }));
      chart.data.datasets[1].data = points.map(({ x, record }) => ({ x, y: record.score.filtered }));
    } else {
      chart.data.datasets[0].data = points.map(({ x, record }) => ({ x, y: record.motion.target_output_deg }));
      chart.data.datasets[1].data = points.map(({ x, record }) => ({ x, y: record.motion.velocity_sps }));
    }
    chart.update('none');
  }

  type ChartPoint = Point;

  function baseOptions(title: string): ChartOptions<'line'> {
    return {
      animation: false, responsive: true, maintainAspectRatio: false, normalized: true,
      interaction: { mode: 'index' as const, intersect: false },
      plugins: { title: { display: true, text: title, align: 'start' as const }, legend: { position: 'bottom' as const } },
      scales: { x: { type: 'linear' as const, title: { display: true, text: 'Device uptime (minutes)' }, ticks: { callback: (value: string | number) => (Number(value) / 60).toFixed(1) } } },
    };
  }

  function scoreConfig(): ChartConfiguration<'line', ChartPoint[]> {
    const base = baseOptions('Alignment score');
    return {
      type: 'line' as const,
      data: { datasets: [
        { label: 'Raw score', data: [] as ChartPoint[], borderColor: '#2563eb', backgroundColor: '#2563eb', pointRadius: 0, borderWidth: 2 },
        { label: 'Filtered score', data: [] as ChartPoint[], borderColor: '#ca8a04', backgroundColor: '#ca8a04', pointRadius: 0, borderWidth: 2 },
      ] },
      options: { ...base, scales: { ...base.scales, y: { min: -5, max: 5, title: { display: true, text: 'Score' } } } },
    };
  }

  function motionConfig(): ChartConfiguration<'line', ChartPoint[]> {
    const base = baseOptions('Output shaft instruction');
    return {
      type: 'line' as const,
      data: { datasets: [
        { label: 'Target output angle', data: [] as ChartPoint[], yAxisID: 'angle', borderColor: '#7c3aed', backgroundColor: '#7c3aed', pointRadius: 0, borderWidth: 2, spanGaps: false },
        { label: 'Commanded speed', data: [] as ChartPoint[], yAxisID: 'speed', borderColor: '#0f766e', backgroundColor: '#0f766e', pointRadius: 0, borderWidth: 2, spanGaps: false },
      ] },
      options: { ...base, scales: { ...base.scales, angle: { type: 'linear' as const, position: 'left' as const, min: -6, max: 6, title: { display: true, text: 'Output angle (°)' } }, speed: { type: 'linear' as const, position: 'right' as const, title: { display: true, text: 'Speed (steps/s)' }, grid: { drawOnChartArea: false } } } },
    };
  }
</script>

<article class="chart-panel"><canvas bind:this={canvas} aria-label={kind === 'score' ? 'Raw and filtered score chart' : 'Output shaft target and speed chart'}></canvas></article>
