<script lang="ts">
  import type { PriceHistory } from '../../lib/api';

  export let history: PriceHistory;

  $: closes = history.prices.map((point) => point.close);
  $: min = Math.min(...closes);
  $: max = Math.max(...closes);
  $: spread = Math.max(max - min, 1);
  $: points = history.prices
    .map((point, index) => {
      const x = history.prices.length === 1 ? 0 : (index / (history.prices.length - 1)) * 100;
      const y = 100 - ((point.close - min) / spread) * 90 - 5;
      return `${x},${y}`;
    })
    .join(' ');
  $: latest = history.prices.at(-1);
</script>

{#if history.prices.length === 0}
  <p class="muted">No prices returned for this ticker.</p>
{:else}
  <div class="chart-card">
    <div class="chart-meta">
      <span>Close</span>
      <strong>{latest?.close.toFixed(2)}</strong>
    </div>
    <svg class="line-chart" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label="One month close price chart">
      <polyline points={points} fill="none" stroke="currentColor" stroke-width="3" vector-effect="non-scaling-stroke" />
    </svg>
    <div class="chart-axis">
      <span>{history.prices[0]?.date}</span>
      <span>{latest?.date}</span>
    </div>
  </div>
{/if}
