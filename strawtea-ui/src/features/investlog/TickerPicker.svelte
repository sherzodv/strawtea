<script context="module" lang="ts">
  export type TickerPickerOption = {
    symbol: string;
    name?: string | null;
    exchange?: string | null;
    asset_type?: string | null;
  };
</script>

<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { chartColorForAssetIndex } from './chartColors';

  export let label = 'Ticker';
  export let selected: string[] = [];
  export let assetTickers: string[] = [];
  export let history: string[] = [];
  export let search: (query: string) => Promise<TickerPickerOption[]> = async () => [];
  export let onSelect: (ticker: string) => void = () => {};
  export let onRemove: (ticker: string) => void = () => {};
  export let onHistoryChange: (tickers: string[]) => void = () => {};

  let query = '';
  let results: TickerPickerOption[] = [];
  let isSearching = false;
  let searchTimer: number | undefined;

  $: normalizedAssetTickers = uniqueSymbols(assetTickers);
  $: visibleRecentTickers = history.filter(
    (ticker) =>
      !selected.includes(ticker) &&
      !normalizedAssetTickers.includes(ticker)
  );
  $: visibleTickers = [
    ...normalizedAssetTickers,
    ...selected.filter((ticker) => !normalizedAssetTickers.includes(ticker)),
    ...visibleRecentTickers
  ];

  $: scheduleSearch(query);

  function scheduleSearch(value: string) {
    window.clearTimeout(searchTimer);
    results = [];

    const trimmed = value.trim();
    if (trimmed.length < 2) {
      isSearching = false;
      return;
    }

    isSearching = true;
    searchTimer = window.setTimeout(async () => {
      try {
        results = await search(trimmed);
      } finally {
        isSearching = false;
      }
    }, 250);
  }

  function choose(ticker: string) {
    const normalized = ticker.trim().toUpperCase();

    if (!normalized) {
      return;
    }

    const nextHistory = [normalized, ...history.filter((item) => item !== normalized)].slice(0, 10);
    onHistoryChange(nextHistory);
    query = '';
    results = [];
    onSelect(normalized);
  }

  function toggleTicker(ticker: string) {
    if (selected.includes(ticker)) {
      onRemove(ticker);
      return;
    }

    choose(ticker);
  }

  function uniqueSymbols(items: string[]) {
    return Array.from(
      new Set(items.map((item) => item.trim().toUpperCase()).filter(Boolean))
    );
  }

  function optionSubtitle(option: TickerPickerOption) {
    return [option.name, option.exchange, option.asset_type].filter(Boolean).join(' · ');
  }
</script>

<div class="stea-picker">
  <label class="stea-field">
    {#if label}
      <span class="stea-field-label">{label}</span>
    {/if}
    <span class="stea-search">
      <Search size={18} />
      <input bind:value={query} type="search" placeholder="Search ticker" aria-label={label || 'Search ticker'} />
    </span>
  </label>

  <div class="stea-picker-body">
    {#if visibleTickers.length > 0}
      <section class="stea-picker-section">
        <p class="stea-picker-heading">Tickers</p>
        <div class="stea-chip-row stea-ticker-chip-row">
          {#each visibleTickers as ticker, index}
            <button
              class="stea-chip stea-chip-ticker"
              class:stea-chip-active={selected.includes(ticker)}
              type="button"
              style={`--stea-chip-color: ${chartColorForAssetIndex(index)}`}
              aria-pressed={selected.includes(ticker)}
              on:click={() => toggleTicker(ticker)}
            >
              {#if normalizedAssetTickers.includes(ticker)}
                <span class="stea-chip-asset-mark" aria-hidden="true"></span>
              {/if}
              {ticker}
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if isSearching}
      <p class="stea-muted">Searching</p>
    {:else if results.length > 0}
      <section class="stea-picker-section">
        <p class="stea-picker-heading">Search results</p>
        <div class="stea-list">
          {#each results as result}
            <button class="stea-list-row" type="button" on:click={() => choose(result.symbol)}>
              <strong>{result.symbol}</strong>
              <span class="stea-list-row-text">{optionSubtitle(result) || result.symbol}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>
