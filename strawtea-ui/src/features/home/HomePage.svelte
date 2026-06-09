<script lang="ts">
  import { ChartCandlestick } from '@lucide/svelte';
  import { route } from '../../lib/router';
</script>

<section class="menu-page">
  <div class="menu-grid" aria-label="Feature menu">
    <button class="app-icon stock-icon" type="button" on:click={() => route.navigate('/stocks')}>
      <span class="icon-art">
        <ChartCandlestick size={42} strokeWidth={1.8} />
      </span>
      <span>Stocks</span>
    </button>
  </div>
</section>
