<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import {
    fetchPriceHistory,
    searchTickers,
    type PriceHistory,
    type TickerSearchResult
  } from '../../lib/api';
  import StockChart from './StockChart.svelte';

  let query = '';
  let results: TickerSearchResult[] = [];
  let selected: TickerSearchResult | null = null;
  let history: PriceHistory | null = null;
  let isSearching = false;
  let isLoadingPrices = false;
  let error = '';
  let timer: number | undefined;

  $: scheduleSearch(query);

  function scheduleSearch(value: string) {
    window.clearTimeout(timer);
    results = [];
    error = '';

    const trimmed = value.trim();
    if (trimmed.length < 2) {
      isSearching = false;
      return;
    }

    isSearching = true;
    timer = window.setTimeout(async () => {
      try {
        results = await searchTickers(trimmed);
      } catch (err) {
        error = err instanceof Error ? err.message : 'Search failed';
      } finally {
        isSearching = false;
      }
    }, 300);
  }

  async function selectTicker(result: TickerSearchResult) {
    selected = result;
    history = null;
    error = '';
    isLoadingPrices = true;

    try {
      history = await fetchPriceHistory(result.symbol);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load prices';
    } finally {
      isLoadingPrices = false;
    }
  }

  onDestroy(() => {
    window.clearTimeout(timer);
  });
</script>

<section class="stea-stack">
  <label class="stea-search">
    <Search size={20} />
    <input bind:value={query} type="search" placeholder="Search ticker, e.g. AAPL" />
  </label>

  {#if error}
    <p class="stea-error">{error}</p>
  {/if}

  {#if isSearching}
    <p class="stea-muted">Searching</p>
  {:else if results.length > 0}
    <div class="stea-list">
      {#each results as result}
        <button class="stea-list-row" type="button" on:click={() => selectTicker(result)}>
          <strong>{result.symbol}</strong>
          <span class="stea-list-row-text">{result.name}</span>
          {#if result.exchange}
            <small class="stea-list-row-meta">{result.exchange}</small>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if selected}
    <section class="stea-stack">
      <div>
        <p class="stea-eyebrow">1 month</p>
        <h2 class="stea-heading">{selected.symbol}</h2>
      </div>
      {#if isLoadingPrices}
        <p class="stea-muted">Loading prices</p>
      {:else if history}
        <StockChart history={history} />
      {/if}
    </section>
  {/if}
</section>
