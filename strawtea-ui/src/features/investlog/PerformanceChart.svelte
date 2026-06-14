<script lang="ts">
  import type { InvestlogPerformance } from '../../lib/api';

  export let performance: InvestlogPerformance;
  export let assetTickers: string[] = [];

  const width = 100;
  const height = 100;
  const colors = ['#315c4d', '#3e6f9f', '#8a4b32', '#7a5ea8', '#5f574d', '#a46a2a', '#2f7771', '#9f2d2d'];

  $: values = performance.series.flatMap((series) => series.points.map((point) => point.index));
  $: min = Math.min(...values, 95);
  $: max = Math.max(...values, 105);
  $: spread = Math.max(max - min, 1);
  $: eventPoints = performance.events
    .map((event) => {
      const series = performance.series.find((item) => item.ticker === event.ticker);
      if (!series || series.points.length === 0) {
        return null;
      }

      const index = series.points.findIndex((point) => point.date >= event.date);
      const point = series.points[index < 0 ? series.points.length - 1 : index];

      if (!point) {
        return null;
      }

      return {
        ...event,
        x: xForPoint(series.points, point.date),
        y: yForValue(point.index)
      };
    })
    .filter(Boolean) as Array<
    {
      ticker: string;
      date: string;
      op: 'buy' | 'sell';
      price: number;
      quantity: number;
      notes: string;
      x: number;
      y: number;
    }
  >;

  function linePoints(points: Array<{ date: string; index: number }>) {
    return points
      .map((point) => `${xForPoint(points, point.date)},${yForValue(point.index)}`)
      .join(' ');
  }

  function xForPoint(points: Array<{ date: string }>, date: string) {
    const index = points.findIndex((point) => point.date === date);
    return points.length === 1 ? 0 : (index / (points.length - 1)) * width;
  }

  function yForValue(value: number) {
    return height - ((value - min) / spread) * 90 - 5;
  }

  function formatMoney(value: number) {
    const amount = (value / 100).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });

    return `${amount} $`;
  }

  function colorForIndex(index: number) {
    return colors[index % colors.length];
  }

  function isAssetTicker(ticker: string) {
    return assetTickers.includes(ticker);
  }
</script>

{#if performance.series.length === 0 || performance.series.every((series) => series.points.length === 0)}
  <p class="stea-muted">No performance data available.</p>
{:else}
  <div class="stea-panel">
    <div class="stea-row-between stea-mb-sm">
      <div>
        <p class="stea-eyebrow">{performance.range.toUpperCase()} performance</p>
        <h2 class="stea-heading-sm">{performance.tickers.join(', ')}</h2>
      </div>
      <div class="stea-legend">
        {#each performance.series as series, index}
          <span class:stea-legend-strong={isAssetTicker(series.ticker)}>
            <i class="stea-swatch" style={`background: ${colorForIndex(index)}`}></i>{series.ticker}
          </span>
        {/each}
      </div>
    </div>

    <div class="stea-chart-wrap">
      <svg class="stea-chart" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label="Ticker versus benchmark performance chart">
        {#each performance.series as series, index}
          <polyline
            points={linePoints(series.points)}
            fill="none"
            stroke={colorForIndex(index)}
            stroke-width={isAssetTicker(series.ticker) ? 3 : 2}
            vector-effect="non-scaling-stroke"
          />
        {/each}
      </svg>
      {#each eventPoints as event}
        <span
          class={event.op === 'buy' ? 'stea-chart-marker stea-chart-marker-buy' : 'stea-chart-marker stea-chart-marker-sell'}
          style={`left: ${event.x}%; top: ${event.y}%;`}
          aria-label={`${event.op} ${event.date}`}
          role="img"
        >
          <span class="stea-sr-only">{event.op.toUpperCase()} {event.date}: {formatMoney(event.price)} - {event.notes}</span>
        </span>
      {/each}
    </div>

    <div class="stea-meta-row stea-meta-row-sm">
      <span>{performance.series[0]?.points[0]?.date}</span>
      <span>{performance.series[0]?.points.at(-1)?.date}</span>
    </div>
  </div>
{/if}
