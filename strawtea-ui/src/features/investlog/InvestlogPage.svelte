<script lang="ts">
  import { ArrowLeft, BookOpen, Plus } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import {
    createInvestlogEntry,
    fetchInvestlogPerformance,
    listInvestlogAssets,
    listInvestlogEntries,
    searchTickers,
    type InvestlogAsset,
    type InvestlogEntry,
    type InvestlogPerformance,
    type InvestlogPerformanceRange
  } from '../../lib/api';
  import { route } from '../../lib/router';
  import PerformanceChart from './PerformanceChart.svelte';
  import TickerPicker from './TickerPicker.svelte';

  let entries: InvestlogEntry[] = [];
  let assets: InvestlogAsset[] = [];
  let ticker = '';
  let occurredAt = defaultLocalDateTime();
  let op: 'buy' | 'sell' = 'buy';
  let price = '';
  let quantity = '';
  let fees = '0';
  let notes = '';
  let error = '';
  let isSaving = false;
  let isLoading = true;
  let isLoadingPerformance = false;
  let isModalOpen = false;
  let selectedAnalysisTickers: string[] = [];
  let performanceRange: InvestlogPerformanceRange = '1y';
  let performance: InvestlogPerformance | null = null;
  let activeTab: 'assets' | 'history' = 'assets';

  $: notesLabel = op === 'buy' ? 'Justify why you bought this' : 'Justify why you sold this';
  $: draftPrice = parseDecimal(price);
  $: draftQuantity = parseDecimal(quantity);
  $: draftFees = parseDecimal(fees || '0');
  $: draftGross = draftPrice * draftQuantity;
  $: draftNet = op === 'buy' ? draftGross + draftFees : draftGross - draftFees;

  onMount(loadEntries);

  async function loadEntries() {
    isLoading = true;
    error = '';

    try {
      [entries, assets] = await Promise.all([listInvestlogEntries(), listInvestlogAssets()]);
      if (selectedAnalysisTickers.length === 0 && assets[0]) {
        selectedAnalysisTickers = [assets[0].ticker];
      }
      await loadPerformance();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load journal';
    } finally {
      isLoading = false;
    }
  }

  async function saveEntry() {
    error = '';

    isSaving = true;

    try {
      const payload = {
        ticker: ticker.trim().toUpperCase(),
        occurred_at: new Date(occurredAt).toISOString(),
        op,
        broker: 'minvest' as const,
        currency: 'USD' as const,
        price: toScaledInteger(price, 100, 'price'),
        quantity: toScaledInteger(quantity, 100, 'quantity'),
        fees: toScaledInteger(fees || '0', 100, 'fees', true),
        notes: notes.trim()
      };

      if (!payload.ticker) {
        throw new Error('Ticker is required');
      }

      if (!payload.notes) {
        throw new Error('Notes are required');
      }

      const created = await createInvestlogEntry(payload);
      entries = [created, ...entries];
      assets = await listInvestlogAssets();
      if (selectedAnalysisTickers.length === 0 && assets[0]) {
        selectedAnalysisTickers = [assets[0].ticker];
      }
      await loadPerformance();
      closeModal();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not save entry';
    } finally {
      isSaving = false;
    }
  }

  function toScaledInteger(value: string, scale: number, label: string, allowZero = false) {
    const parsed = Number(value);

    if (!Number.isFinite(parsed) || parsed < 0 || (!allowZero && parsed === 0)) {
      throw new Error(`${label} must be a valid number`);
    }

    return Math.round(parsed * scale);
  }

  function defaultLocalDateTime() {
    const now = new Date();
    now.setSeconds(0, 0);
    return toLocalDateTimeInputValue(now);
  }

  function toLocalDateTimeInputValue(date: Date) {
    const offsetMs = date.getTimezoneOffset() * 60_000;
    return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
  }

  function formatMoney(value: number) {
    const amount = (value / 100).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });

    return `${amount} $`;
  }

  function formatQuantity(value: number) {
    return (value / 100).toLocaleString(undefined, {
      minimumFractionDigits: 0,
      maximumFractionDigits: 2
    });
  }

  function parseDecimal(value: string) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function grossCents(entry: InvestlogEntry) {
    return Math.round((entry.price * entry.quantity) / 100);
  }

  function netCents(entry: InvestlogEntry) {
    const gross = grossCents(entry);
    return entry.op === 'buy' ? gross + entry.fees : gross - entry.fees;
  }

  function formatDraftMoney(value: number) {
    const amount = value.toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });

    return `${amount} $`;
  }

  function openModal() {
    resetForm();
    isModalOpen = true;
  }

  function closeModal() {
    isModalOpen = false;
    resetForm();
  }

  function resetForm() {
    ticker = '';
    occurredAt = defaultLocalDateTime();
    op = 'buy';
    price = '';
    quantity = '';
    fees = '0';
    notes = '';
    error = '';
  }

  async function loadPerformance() {
    if (selectedAnalysisTickers.length === 0) {
      performance = null;
      return;
    }

    isLoadingPerformance = true;

    try {
      performance = await fetchInvestlogPerformance(selectedAnalysisTickers, performanceRange);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load performance';
    } finally {
      isLoadingPerformance = false;
    }
  }

  function formatPercent(value: number) {
    return `${value.toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    })}%`;
  }

  function formatDays(value: number) {
    return value.toLocaleString();
  }

  function changeClass(value: number) {
    if (value > 0) {
      return 'stea-gain';
    }

    if (value < 0) {
      return 'stea-loss';
    }

    return '';
  }

  function addAnalysisTicker(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    if (!normalized || selectedAnalysisTickers.includes(normalized)) {
      return;
    }

    selectedAnalysisTickers = [...selectedAnalysisTickers, normalized];
    loadPerformance();
  }

  function removeAnalysisTicker(ticker: string) {
    selectedAnalysisTickers = selectedAnalysisTickers.filter((item) => item !== ticker);
    loadPerformance();
  }
</script>

<section class="stea-stack-lg">
  <div class="stea-row">
    <button class="stea-icon-btn" type="button" aria-label="Back home" on:click={() => route.navigate('/')}>
      <ArrowLeft size={20} />
    </button>
    <div>
      <p class="stea-eyebrow">Investment Journal</p>
      <h1 class="stea-heading">Investlog</h1>
    </div>
  </div>

  <div class="stea-row-between">
    <p class="stea-muted">Track buys and sells with the reason behind each decision.</p>
    <button class="stea-btn-primary stea-btn-fit" type="button" on:click={openModal}>
      <Plus size={20} />
      Add entry
    </button>
  </div>

  {#if isModalOpen}
    <div class="stea-modal-backdrop">
      <div class="stea-modal" role="dialog" aria-modal="true" aria-labelledby="investlog-modal-title" tabindex="-1">
        <div class="stea-modal-header">
          <div>
            <p class="stea-eyebrow">Investlog</p>
            <h2 id="investlog-modal-title" class="stea-heading-sm">Add entry</h2>
          </div>
          <button class="stea-icon-btn" type="button" aria-label="Close" on:click={closeModal}>×</button>
        </div>

        <form class="stea-form-grid" on:submit|preventDefault={saveEntry}>
          <label class="stea-field">
            <span class="stea-field-label">Ticker</span>
            <input class="stea-input" bind:value={ticker} type="text" placeholder="AAPL" autocomplete="off" />
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Date and time</span>
            <input class="stea-input" bind:value={occurredAt} type="datetime-local" />
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Operation</span>
            <select class="stea-input" bind:value={op}>
              <option value="buy">Buy</option>
              <option value="sell">Sell</option>
            </select>
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Broker</span>
            <select class="stea-input" value="minvest" disabled>
              <option value="minvest">Minvest</option>
            </select>
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Price USD</span>
            <input class="stea-input" bind:value={price} type="number" min="0.01" step="0.01" placeholder="100.00" />
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Quantity</span>
            <input class="stea-input" bind:value={quantity} type="number" min="0.01" step="0.01" placeholder="1" />
          </label>

          <label class="stea-field">
            <span class="stea-field-label">Fees USD</span>
            <input class="stea-input" bind:value={fees} type="number" min="0" step="0.01" />
          </label>

          <dl class="stea-stat-grid stea-stat-strip stea-span-all">
            <div>
              <dt>Gross value</dt>
              <dd>{formatDraftMoney(draftGross)}</dd>
            </div>
            <div>
              <dt>Fees</dt>
              <dd>{formatDraftMoney(draftFees)}</dd>
            </div>
            <div>
              <dt>{op === 'buy' ? 'Total cost' : 'Net proceeds'}</dt>
              <dd>{formatDraftMoney(draftNet)}</dd>
            </div>
          </dl>

          <label class="stea-field stea-span-all">
            <span class="stea-field-label">{notesLabel}</span>
            <textarea class="stea-input stea-textarea" bind:value={notes} rows="4"></textarea>
          </label>

          {#if error}
            <p class="stea-error stea-span-all">{error}</p>
          {/if}

          <div class="stea-modal-actions stea-span-all">
            <button class="stea-btn-secondary" type="button" on:click={closeModal}>Cancel</button>
            <button class="stea-btn-primary stea-btn-fit" type="submit" disabled={isSaving}>
              <Plus size={20} />
              {isSaving ? 'Saving' : 'Save entry'}
            </button>
          </div>
        </form>
      </div>
    </div>
  {/if}

  <div class="stea-tabs" role="tablist" aria-label="Investlog views">
    <button
      class={activeTab === 'assets' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'assets'}
      on:click={() => (activeTab = 'assets')}
    >
      Assets
    </button>
    <button
      class={activeTab === 'history' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'history'}
      on:click={() => (activeTab = 'history')}
    >
      History
    </button>
  </div>

  {#if activeTab === 'assets'}
    <section class="stea-stack" aria-label="Current investment assets">
      <div class="stea-row">
        <BookOpen size={20} />
        <h2 class="stea-heading-sm">Assets</h2>
      </div>

      {#if isLoading}
        <p class="stea-muted">Loading assets</p>
      {:else if assets.length === 0}
        <p class="stea-muted">No current assets.</p>
      {:else}
        <div class="stea-table-wrap">
          <table class="stea-table">
            <thead>
              <tr>
                <th>Ticker</th>
                <th>Days since mid buys</th>
                <th>Change %</th>
                <th>Change</th>
                <th>Cost</th>
                <th>Qty</th>
                <th>Price</th>
                <th>Current</th>
                <th>Price change</th>
              </tr>
            </thead>
            <tbody>
              {#each assets as asset}
                <tr>
                  <td><strong>{asset.ticker}</strong></td>
                  <td>{formatDays(asset.days_since_buy_midpoint)}</td>
                  <td class={changeClass(asset.percent_change)}>{formatPercent(asset.percent_change)}</td>
                  <td class={changeClass(asset.amount_change)}>{formatMoney(asset.amount_change)}</td>
                  <td>{formatMoney(asset.cost)}</td>
                  <td>{formatQuantity(asset.quantity)}</td>
                  <td>{formatMoney(asset.avg_buy_price)}</td>
                  <td>{formatMoney(asset.current_price)}</td>
                  <td class={changeClass(asset.price_change)}>{formatMoney(asset.price_change)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>

    <section class="stea-stack" aria-label="Ticker analysis against benchmark">
      <div class="stea-row-between">
        <div class="stea-row">
          <BookOpen size={20} />
          <h2 class="stea-heading-sm">Analysis</h2>
        </div>
        <div class="stea-toolbar">
          <label class="stea-field">
            <span class="stea-field-label">Range</span>
            <select class="stea-input" bind:value={performanceRange} on:change={loadPerformance}>
              <option value="1m">1M</option>
              <option value="3m">3M</option>
              <option value="6m">6M</option>
              <option value="1y">1Y</option>
              <option value="3y">3Y</option>
            </select>
          </label>
        </div>
      </div>

      <TickerPicker
        label="Add ticker"
        selected={selectedAnalysisTickers}
        assetTickers={assets.map((asset) => asset.ticker)}
        historyKey="strawtea:analysis-ticker-history"
        search={searchTickers}
        onSelect={addAnalysisTicker}
      />

      {#if selectedAnalysisTickers.length > 0}
        <div class="stea-chip-row" aria-label="Selected analysis tickers">
          {#each selectedAnalysisTickers as ticker}
            <button class="stea-chip stea-chip-active" type="button" on:click={() => removeAnalysisTicker(ticker)}>
              {ticker} ×
            </button>
          {/each}
        </div>
      {/if}

      {#if isLoadingPerformance}
        <p class="stea-muted">Loading performance</p>
      {:else if performance}
        <PerformanceChart {performance} assetTickers={assets.map((asset) => asset.ticker)} />
      {:else}
        <p class="stea-muted">Add a ticker to start analysis.</p>
      {/if}
    </section>
  {:else}
    <section class="stea-stack" aria-label="Investment journal history">
      <div class="stea-row">
        <BookOpen size={20} />
        <h2 class="stea-heading-sm">History</h2>
      </div>

      {#if isLoading}
        <p class="stea-muted">Loading history</p>
      {:else if entries.length === 0}
        <p class="stea-muted">No entries yet.</p>
      {:else}
        {#each entries as entry}
          <article class="stea-panel-grid">
            <div class="stea-row-between">
              <strong>{entry.ticker}</strong>
              <span class={entry.op === 'buy' ? 'stea-badge stea-badge-buy' : 'stea-badge stea-badge-sell'}>{entry.op}</span>
            </div>
            <p class="stea-note">{new Date(entry.occurred_at).toLocaleString()}</p>
            <dl class="stea-stat-grid">
              <div>
                <dt>Price</dt>
                <dd>{formatMoney(entry.price)}</dd>
              </div>
              <div>
                <dt>Qty</dt>
                <dd>{formatQuantity(entry.quantity)}</dd>
              </div>
              <div>
                <dt>Fees</dt>
                <dd>{formatMoney(entry.fees)}</dd>
              </div>
              <div>
                <dt>Gross</dt>
                <dd>{formatMoney(grossCents(entry))}</dd>
              </div>
              <div>
                <dt>{entry.op === 'buy' ? 'Total cost' : 'Net proceeds'}</dt>
                <dd>{formatMoney(netCents(entry))}</dd>
              </div>
            </dl>
            <p class="stea-note">{entry.notes}</p>
          </article>
        {/each}
      {/if}
    </section>
  {/if}
</section>
