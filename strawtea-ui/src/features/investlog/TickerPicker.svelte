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
  import { onMount } from 'svelte';

  export let label = 'Ticker';
  export let selected: string[] = [];
  export let assetTickers: string[] = [];
  export let historyKey = 'strawtea:ticker-picker-history';
  export let search: (query: string) => Promise<TickerPickerOption[]> = async () => [];
  export let onSelect: (ticker: string) => void = () => {};

  let query = '';
  let history: string[] = [];
  let results: TickerPickerOption[] = [];
  let isSearching = false;
  let searchTimer: number | undefined;

  $: normalizedAssetTickers = uniqueSymbols(assetTickers);
  $: visibleHistory = history.filter(
    (ticker) =>
      !selected.includes(ticker) &&
      !normalizedAssetTickers.includes(ticker)
  );

  $: scheduleSearch(query);

  onMount(() => {
    history = readHistory();
  });

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

    history = [normalized, ...history.filter((item) => item !== normalized)].slice(0, 10);
    writeHistory(history);
    query = '';
    results = [];
    onSelect(normalized);
  }

  function readHistory() {
    try {
      const parsed = JSON.parse(localStorage.getItem(historyKey) ?? '[]');
      return Array.isArray(parsed) ? uniqueSymbols(parsed).slice(0, 10) : [];
    } catch {
      return [];
    }
  }

  function writeHistory(items: string[]) {
    localStorage.setItem(historyKey, JSON.stringify(items));
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
    <span class="stea-field-label">{label}</span>
    <span class="stea-search">
      <Search size={18} />
      <input bind:value={query} type="search" placeholder="Search ticker" />
    </span>
  </label>

  <div class="stea-picker-body">
    {#if normalizedAssetTickers.length > 0}
      <section class="stea-picker-section">
        <p class="stea-picker-heading">Assets</p>
        <div class="stea-chip-row">
          {#each normalizedAssetTickers as ticker}
            <button class="stea-chip stea-chip-asset" class:stea-chip-active={selected.includes(ticker)} type="button" on:click={() => choose(ticker)}>{ticker}</button>
          {/each}
        </div>
      </section>
    {/if}

    {#if visibleHistory.length > 0}
      <section class="stea-picker-section stea-picker-section-divided">
        <p class="stea-picker-heading">Recent</p>
        <div class="stea-chip-row">
          {#each visibleHistory as ticker}
            <button class="stea-chip" class:stea-chip-active={selected.includes(ticker)} type="button" on:click={() => choose(ticker)}>{ticker}</button>
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
