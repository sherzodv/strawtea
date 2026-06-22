<script lang="ts">
  import { BookOpen, ExternalLink, MessageSquare, Pencil, Plus, Star, StarOff } from '@lucide/svelte';
  import { onDestroy, onMount } from 'svelte';
  import {
    addInvestlogWatchlistItem,
    createInvestlogEntry,
    createInvestlogTickerNote,
    fetchCompanyProfile,
    fetchInvestlogPerformance,
    listInvestlogAssets,
    listInvestlogEntries,
    listInvestlogWatchlist,
    removeInvestlogWatchlistItem,
    searchTickers,
    updateInvestlogEntry,
    type CompanyAddress,
    type CompanyFinancialMetric,
    type CompanyProfile,
    type InvestlogAsset,
    type InvestlogAssetsSummary,
    type InvestlogEntry,
    type InvestlogPerformance,
    type InvestlogPerformanceRange,
    type InvestlogTickerNote,
    type InvestlogWatchlistItem
  } from '../../lib/api';
  import {
    appSettings,
    type AnalysisIntervalSetting,
    type InvestlogAnalysisSettings,
    type InvestlogTab
  } from '../../lib/settings';
  import AiCorrectionsPage from '../ai-corrections/AiCorrectionsPage.svelte';
  import PerformanceChart from './PerformanceChart.svelte';
  import TickerPicker from './TickerPicker.svelte';

  type TickerNotesView = {
    ticker: string;
    company_name: string | null;
    notes: InvestlogTickerNote[];
  };

  let entries: InvestlogEntry[] = [];
  let assets: InvestlogAsset[] = [];
  let assetsSummary: InvestlogAssetsSummary | null = null;
  let watchlist: InvestlogWatchlistItem[] = [];
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
  let editingEntryId: string | null = null;
  let isProfileModalOpen = false;
  let selectedTickerNotes: TickerNotesView | null = null;
  let isLoadingProfile = false;
  let profileError = '';
  let companyProfile: CompanyProfile | null = null;
  let selectedAnalysisTickers: string[] = [];
  let recentAnalysisTickers: string[] = [];
  let performanceRange: InvestlogPerformanceRange = '6m';
  let analysisInterval: AnalysisIntervalSetting | null = null;
  let performance: InvestlogPerformance | null = null;
  let activeTab: InvestlogTab = 'assets';
  let profileRequestId = 0;
  const profileTapTimers = new Map<string, number>();
  const doubleTapMs = 280;
  const performanceRanges: Array<{ value: InvestlogPerformanceRange; label: string }> = [
    { value: '1m', label: '1M' },
    { value: '3m', label: '3M' },
    { value: '6m', label: '6M' },
    { value: '1y', label: '1Y' },
    { value: '3y', label: '3Y' }
  ];

  $: notesLabel = op === 'buy' ? 'Justify why you bought this' : 'Justify why you sold this';
  $: assetTickers = assets.map((asset) => asset.ticker);
  $: activeWatchlistTickers = watchlist
    .filter((item) => item.is_active)
    .map((item) => item.ticker);
  $: selectedTickerNotesForAnalysis = selectedAnalysisTickers.map((ticker) => {
    const watchlistItem = watchlist.find((item) => item.ticker === ticker);
    return {
      ticker,
      company_name: watchlistItem?.company_name ?? null,
      notes: performance?.ticker_notes.filter((note) => note.ticker === ticker) ?? []
    };
  });
  $: analysisColorTickers = [
    ...assetTickers,
    ...activeWatchlistTickers.filter((ticker) => !assetTickers.includes(ticker)),
    ...selectedAnalysisTickers.filter(
      (ticker) =>
        !assetTickers.includes(ticker) &&
        !activeWatchlistTickers.includes(ticker)
    )
  ];
  $: draftPrice = parseDecimal(price);
  $: draftQuantity = parseDecimal(quantity);
  $: draftFees = parseDecimal(fees || '0');
  $: draftGross = draftPrice * draftQuantity;
  $: draftNet = op === 'buy' ? draftGross + draftFees : draftGross - draftFees;

  onMount(loadInvestlog);

  onDestroy(() => {
    profileTapTimers.forEach((timer) => window.clearTimeout(timer));
    profileTapTimers.clear();
  });

  async function loadInvestlog() {
    isLoading = true;
    error = '';

    try {
      const [
        pageSettings,
        analysisSettings,
        loadedEntries,
        loadedAssets,
        loadedWatchlist
      ] = await Promise.all([
        appSettings.investlog.page.load(),
        appSettings.investlog.analysis.load(),
        listInvestlogEntries(),
        listInvestlogAssets(),
        listInvestlogWatchlist()
      ]);

      activeTab = normalizeTab(pageSettings.activeTab);
      selectedAnalysisTickers = uniqueSymbols(analysisSettings.selectedTickers);
      recentAnalysisTickers = uniqueSymbols(
        analysisSettings.recentTickers.length > 0
          ? analysisSettings.recentTickers
          : analysisSettings.tickerHistory ?? readLegacyAnalysisTickerHistory()
      ).slice(0, 10);
      performanceRange = normalizePerformanceRange(analysisSettings.range);
      analysisInterval = normalizeInterval(analysisSettings.interval);
      entries = loadedEntries;
      assets = loadedAssets.assets;
      assetsSummary = loadedAssets.summary;
      watchlist = loadedWatchlist;

      if (selectedAnalysisTickers.length === 0 && assets[0]) {
        selectedAnalysisTickers = [assets[0].ticker];
        saveAnalysisSettings();
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

      if (editingEntryId) {
        await updateInvestlogEntry(editingEntryId, payload);
        entries = await listInvestlogEntries();
      } else {
        const created = await createInvestlogEntry(payload);
        entries = [created, ...entries];
      }
      const loadedAssets = await listInvestlogAssets();
      assets = loadedAssets.assets;
      assetsSummary = loadedAssets.summary;
      if (selectedAnalysisTickers.length === 0 && assets[0]) {
        selectedAnalysisTickers = [assets[0].ticker];
        saveAnalysisSettings();
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

  function formatCompactMoney(value: number | null) {
    if (value == null) return '-';
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency: 'USD',
      notation: 'compact',
      maximumFractionDigits: 1
    }).format(value / 100);
  }

  function formatShares(value: number | null) {
    if (value == null) return '-';
    return new Intl.NumberFormat(undefined, {
      notation: 'compact',
      maximumFractionDigits: 1
    }).format(value);
  }

  function formatOptionalMoney(value: number | null) {
    return value == null ? '-' : formatMoney(value);
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

  function formatInputAmount(value: number, scale = 100) {
    return (value / scale).toLocaleString('en-US', {
      useGrouping: false,
      minimumFractionDigits: 0,
      maximumFractionDigits: 2
    });
  }

  function openModal() {
    resetForm();
    isModalOpen = true;
  }

  function openEditEntry(entry: InvestlogEntry) {
    editingEntryId = entry.id;
    ticker = entry.ticker;
    occurredAt = toLocalDateTimeInputValue(new Date(entry.occurred_at));
    op = entry.op;
    price = formatInputAmount(entry.price);
    quantity = formatInputAmount(entry.quantity);
    fees = formatInputAmount(entry.fees);
    notes = entry.notes;
    error = '';
    isModalOpen = true;
  }

  function closeModal() {
    isModalOpen = false;
    resetForm();
  }

  async function openCompanyProfile(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    if (!normalized) {
      return;
    }

    const requestId = ++profileRequestId;
    isProfileModalOpen = true;
    isLoadingProfile = true;
    profileError = '';
    companyProfile = null;

    try {
      const profile = await fetchCompanyProfile(normalized);
      if (requestId === profileRequestId) {
        companyProfile = profile;
      }
    } catch (err) {
      if (requestId === profileRequestId) {
        profileError = err instanceof Error ? err.message : 'Could not load company profile';
      }
    } finally {
      if (requestId === profileRequestId) {
        isLoadingProfile = false;
      }
    }
  }

  function closeCompanyProfile() {
    isProfileModalOpen = false;
    profileError = '';
    companyProfile = null;
  }

  function handleTickerProfileTap(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    const existingTimer = profileTapTimers.get(normalized);

    if (existingTimer) {
      window.clearTimeout(existingTimer);
      profileTapTimers.delete(normalized);
      openCompanyProfile(normalized);
      return;
    }

    const timer = window.setTimeout(() => {
      profileTapTimers.delete(normalized);
    }, doubleTapMs);
    profileTapTimers.set(normalized, timer);
  }

  function resetForm() {
    ticker = '';
    occurredAt = defaultLocalDateTime();
    op = 'buy';
    price = '';
    quantity = '';
    fees = '0';
    notes = '';
    editingEntryId = null;
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

  function formatDate(value: string | null) {
    if (!value) return '-';
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(new Date(value));
  }

  function formatNoteCount(count: number) {
    return `${count.toLocaleString()} ${count === 1 ? 'note' : 'notes'}`;
  }

  function formatDays(value: number) {
    return value.toLocaleString();
  }

  function formatFiscalYearEnd(value: string | null) {
    if (!value || value.length !== 4) {
      return value;
    }

    return `${value.slice(0, 2)}/${value.slice(2)}`;
  }

  function addressLines(address: CompanyAddress | null) {
    if (!address) {
      return [];
    }

    return [
      address.street1,
      address.street2,
      [address.city, address.state_or_country, address.zip_code].filter(Boolean).join(', ')
    ].filter(Boolean);
  }

  function joinValues(values: string[]) {
    return values.length > 0 ? values.join(', ') : '—';
  }

  function formatFinancialValue(metric: CompanyFinancialMetric) {
    if (metric.unit === 'USD') {
      return formatLargeUsd(metric.value);
    }

    if (metric.unit === 'USD/shares') {
      return `$${metric.value.toLocaleString(undefined, {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2
      })}`;
    }

    if (metric.unit === 'shares') {
      return formatLargeNumber(metric.value);
    }

    return metric.value.toLocaleString();
  }

  function formatLargeUsd(value: number) {
    return `$${formatLargeNumber(value)}`;
  }

  function formatLargeNumber(value: number) {
    const absolute = Math.abs(value);
    const sign = value < 0 ? '-' : '';

    if (absolute >= 1_000_000_000_000) {
      return `${sign}${(absolute / 1_000_000_000_000).toLocaleString(undefined, {
        maximumFractionDigits: 2
      })}T`;
    }

    if (absolute >= 1_000_000_000) {
      return `${sign}${(absolute / 1_000_000_000).toLocaleString(undefined, {
        maximumFractionDigits: 2
      })}B`;
    }

    if (absolute >= 1_000_000) {
      return `${sign}${(absolute / 1_000_000).toLocaleString(undefined, {
        maximumFractionDigits: 2
      })}M`;
    }

    return value.toLocaleString(undefined, {
      maximumFractionDigits: 2
    });
  }

  function metricPeriod(metric: CompanyFinancialMetric) {
    return [metric.form, metric.fiscal_year, metric.fiscal_period, metric.end]
      .filter(Boolean)
      .join(' · ');
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
    analysisInterval = null;
    saveAnalysisSettings();
    loadPerformance();
  }

  function removeAnalysisTicker(ticker: string) {
    selectedAnalysisTickers = selectedAnalysisTickers.filter((item) => item !== ticker);
    analysisInterval = null;
    saveAnalysisSettings();
    loadPerformance();
  }

  async function addToWatchlist(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    if (!normalized) return;

    const note = window.prompt(`Why add ${normalized} to watchlist?`);
    if (note === null) return;
    if (!note.trim()) {
      error = 'Watchlist note is required';
      return;
    }

    error = '';
    try {
      await addInvestlogWatchlistItem({ ticker: normalized, note });
      watchlist = await listInvestlogWatchlist();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not add to watchlist';
    }
  }

  async function removeFromWatchlist(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    if (!normalized) return;

    const note = window.prompt(`Why remove ${normalized} from watchlist?`);
    if (note === null) return;
    if (!note.trim()) {
      error = 'Watchlist removal note is required';
      return;
    }

    error = '';
    try {
      await removeInvestlogWatchlistItem(normalized, { note });
      watchlist = await listInvestlogWatchlist();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not remove from watchlist';
    }
  }

  function watchlistActionLabel(ticker: string) {
    return activeWatchlistTickers.includes(ticker) ? 'Remove watchlist' : 'Add watchlist';
  }

  function toggleWatchlist(ticker: string) {
    if (activeWatchlistTickers.includes(ticker)) {
      removeFromWatchlist(ticker);
      return;
    }

    addToWatchlist(ticker);
  }

  function openTickerNotes(item: TickerNotesView) {
    selectedTickerNotes = item;
  }

  function closeTickerNotes() {
    selectedTickerNotes = null;
  }

  function watchlistTickerNotesView(item: InvestlogWatchlistItem): TickerNotesView {
    return {
      ticker: item.ticker,
      company_name: item.company_name,
      notes: item.notes
    };
  }

  async function addTickerNote(ticker: string) {
    const normalized = ticker.trim().toUpperCase();
    if (!normalized) return;

    const note = window.prompt(`Add note for ${normalized}`);
    if (note === null) return;
    if (!note.trim()) {
      error = 'Ticker note is required';
      return;
    }

    error = '';
    try {
      const created = await createInvestlogTickerNote({ ticker: normalized, note });
      applyTickerNote(created);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not add ticker note';
    }
  }

  function addSelectedTickerNote() {
    if (!selectedTickerNotes) return;
    addTickerNote(selectedTickerNotes.ticker);
  }

  function applyTickerNote(note: InvestlogTickerNote) {
    watchlist = watchlist.map((item) =>
      item.ticker === note.ticker ? { ...item, notes: [note, ...item.notes] } : item
    );

    if (performance && performance.tickers.includes(note.ticker)) {
      performance = {
        ...performance,
        ticker_notes: [
          note,
          ...performance.ticker_notes.filter((item) => item.id !== note.id)
        ]
      };
    }

    if (selectedTickerNotes?.ticker === note.ticker) {
      selectedTickerNotes = {
        ...selectedTickerNotes,
        notes: [note, ...selectedTickerNotes.notes]
      };
    }
  }

  async function addSelectedToWatchlist() {
    const tickers = selectedAnalysisTickers.filter(
      (ticker) => !activeWatchlistTickers.includes(ticker)
    );
    if (tickers.length === 0) return;

    const note = window.prompt(`Why add ${tickers.join(', ')} to watchlist?`);
    if (note === null) return;
    if (!note.trim()) {
      error = 'Watchlist note is required';
      return;
    }

    error = '';
    try {
      for (const ticker of tickers) {
        await addInvestlogWatchlistItem({ ticker, note });
      }
      watchlist = await listInvestlogWatchlist();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not add selected tickers to watchlist';
    }
  }

  function setAnalysisTickerHistory(tickers: string[]) {
    recentAnalysisTickers = uniqueSymbols(tickers).slice(0, 10);
    saveAnalysisSettings();
  }

  function selectPerformanceRange(range: InvestlogPerformanceRange) {
    if (performanceRange === range) {
      return;
    }

    performanceRange = range;
    saveAnalysisSettings();
    loadPerformance();
  }

  function selectActiveTab(tab: InvestlogTab) {
    activeTab = tab;
    appSettings.investlog.page.save({ activeTab });
  }

  function setAnalysisInterval(interval: AnalysisIntervalSetting | null) {
    analysisInterval = interval;
    saveAnalysisSettings();
  }

  function saveAnalysisSettings() {
    appSettings.investlog.analysis.save(currentAnalysisSettings());
  }

  function currentAnalysisSettings(): InvestlogAnalysisSettings {
    return {
      selectedTickers: selectedAnalysisTickers,
      recentTickers: recentAnalysisTickers,
      range: performanceRange,
      interval: analysisInterval
    };
  }

  function normalizeTab(value: string): InvestlogTab {
    return value === 'analysis' ||
      value === 'screener' ||
      value === 'watchlist' ||
      value === 'history'
      ? value
      : 'assets';
  }

  function normalizePerformanceRange(value: string): InvestlogPerformanceRange {
    return performanceRanges.some((range) => range.value === value)
      ? (value as InvestlogPerformanceRange)
      : '6m';
  }

  function normalizeInterval(value: AnalysisIntervalSetting | null | undefined): AnalysisIntervalSetting | null {
    if (!value?.anchorDate || !value.compareDate) {
      return null;
    }

    return {
      anchorDate: value.anchorDate,
      compareDate: value.compareDate,
      nextTarget: value.nextTarget === 'compare' ? 'compare' : 'anchor'
    };
  }

  function readLegacyAnalysisTickerHistory() {
    try {
      const parsed = JSON.parse(localStorage.getItem('strawtea:analysis-ticker-history') ?? '[]');
      return Array.isArray(parsed) ? uniqueSymbols(parsed).slice(0, 10) : [];
    } catch {
      return [];
    }
  }

  function uniqueSymbols(items: string[]) {
    return Array.from(
      new Set(items.map((item) => item.trim().toUpperCase()).filter(Boolean))
    );
  }
</script>

<section class="stea-stack-lg">
  {#if isModalOpen}
    <div class="stea-modal-backdrop" role="presentation" on:pointerdown={closeModal}>
      <div class="stea-modal" role="dialog" aria-modal="true" aria-labelledby="investlog-modal-title" tabindex="-1" on:pointerdown|stopPropagation>
        <div class="stea-modal-header">
          <div>
            <p class="stea-eyebrow">Investlog</p>
            <h2 id="investlog-modal-title" class="stea-heading-sm">{editingEntryId ? 'Edit entry' : 'Add entry'}</h2>
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
              {#if editingEntryId}
                <Pencil size={20} />
              {:else}
                <Plus size={20} />
              {/if}
              {isSaving ? 'Saving' : editingEntryId ? 'Save changes' : 'Save entry'}
            </button>
          </div>
        </form>
      </div>
    </div>
  {/if}

  {#if isProfileModalOpen}
    <div class="stea-modal-backdrop" role="presentation" on:pointerdown={closeCompanyProfile}>
      <div class="stea-modal stea-profile-modal" role="dialog" aria-modal="true" aria-labelledby="company-profile-title" tabindex="-1" on:pointerdown|stopPropagation>
        <div class="stea-modal-header">
          <div>
            <p class="stea-eyebrow">SEC EDGAR</p>
            <h2 id="company-profile-title" class="stea-heading-sm">
              {companyProfile?.ticker ?? 'Company profile'}
            </h2>
          </div>
          <button class="stea-icon-btn" type="button" aria-label="Close" on:click={closeCompanyProfile}>×</button>
        </div>

        {#if isLoadingProfile}
          <p class="stea-muted">Loading company profile</p>
        {:else if profileError}
          <p class="stea-error">{profileError}</p>
        {:else if companyProfile}
          <div class="stea-stack">
            <div>
              <h3 class="stea-heading">{companyProfile.name}</h3>
              <p class="stea-note">
                CIK {companyProfile.cik}
                {#if companyProfile.entity_type}
                  · {companyProfile.entity_type}
                {/if}
              </p>
            </div>

            <dl class="stea-stat-grid stea-profile-grid">
              <div>
                <dt>Exchange</dt>
                <dd>{joinValues(companyProfile.exchanges)}</dd>
              </div>
              <div>
                <dt>Tickers</dt>
                <dd>{joinValues(companyProfile.tickers)}</dd>
              </div>
              <div>
                <dt>SIC</dt>
                <dd>
                  {#if companyProfile.sic || companyProfile.sic_description}
                    {[companyProfile.sic, companyProfile.sic_description].filter(Boolean).join(' · ')}
                  {:else}
                    —
                  {/if}
                </dd>
              </div>
              <div>
                <dt>Fiscal year end</dt>
                <dd>{formatFiscalYearEnd(companyProfile.fiscal_year_end) ?? '—'}</dd>
              </div>
              <div>
                <dt>State</dt>
                <dd>{companyProfile.state_of_incorporation ?? '—'}</dd>
              </div>
              <div>
                <dt>Phone</dt>
                <dd>{companyProfile.phone ?? '—'}</dd>
              </div>
            </dl>

            <div class="stea-profile-address-grid">
              <section>
                <p class="stea-picker-heading">Business address</p>
                {#if addressLines(companyProfile.business_address).length > 0}
                  {#each addressLines(companyProfile.business_address) as line}
                    <p class="stea-note">{line}</p>
                  {/each}
                {:else}
                  <p class="stea-muted">—</p>
                {/if}
              </section>
              <section>
                <p class="stea-picker-heading">Mailing address</p>
                {#if addressLines(companyProfile.mailing_address).length > 0}
                  {#each addressLines(companyProfile.mailing_address) as line}
                    <p class="stea-note">{line}</p>
                  {/each}
                {:else}
                  <p class="stea-muted">—</p>
                {/if}
              </section>
            </div>

            {#if companyProfile.financials.length > 0}
              <section class="stea-stack">
                <p class="stea-picker-heading">Financials</p>
                <div class="stea-financial-grid">
                  {#each companyProfile.financials as metric}
                    <article class="stea-financial-item">
                      <p class="stea-financial-label">{metric.label}</p>
                      <strong>{formatFinancialValue(metric)}</strong>
                      <span>{metricPeriod(metric)}</span>
                      <small>{metric.concept}</small>
                    </article>
                  {/each}
                </div>
              </section>
            {/if}

            {#if companyProfile.recent_filings.length > 0}
              <section class="stea-stack">
                <p class="stea-picker-heading">Recent filings</p>
                <div class="stea-list">
                  {#each companyProfile.recent_filings as filing}
                    <a class="stea-list-row stea-profile-filing" href={filing.url ?? companyProfile.sec_url} target="_blank" rel="noreferrer">
                      <strong>{filing.form}</strong>
                      <span class="stea-list-row-text">{filing.description ?? filing.primary_document ?? filing.accession_number ?? 'Filing'}</span>
                      <span class="stea-list-row-meta">{filing.filing_date ?? '—'}</span>
                    </a>
                  {/each}
                </div>
              </section>
            {/if}

            <a class="stea-btn-secondary stea-btn-fit" href={companyProfile.sec_url} target="_blank" rel="noreferrer">
              <ExternalLink size={18} />
              Open in SEC
            </a>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if selectedTickerNotes}
    <div class="stea-modal-backdrop" role="presentation" on:pointerdown={closeTickerNotes}>
      <div class="stea-modal" role="dialog" aria-modal="true" aria-labelledby="watchlist-notes-title" tabindex="-1" on:pointerdown|stopPropagation>
        <div class="stea-modal-header">
          <div>
            <p class="stea-eyebrow">Ticker notes</p>
            <h2 id="watchlist-notes-title" class="stea-heading-sm">
              {selectedTickerNotes.ticker}
              {#if selectedTickerNotes.company_name}
                · {selectedTickerNotes.company_name}
              {/if}
            </h2>
          </div>
          <button class="stea-icon-btn" type="button" aria-label="Close" on:click={closeTickerNotes}>×</button>
        </div>

        {#if selectedTickerNotes.notes.length === 0}
          <p class="stea-muted">No notes yet.</p>
        {:else}
          <div class="stea-watchlist-notes">
            {#each selectedTickerNotes.notes as note}
              <article class="stea-watchlist-note">
                <p>{note.note}</p>
                <span>{formatDate(note.created_at)}</span>
              </article>
            {/each}
          </div>
        {/if}

        <div class="stea-modal-actions">
          <button class="stea-btn-secondary stea-btn-fit" type="button" on:click={addSelectedTickerNote}>
            <MessageSquare size={18} />
            Add note
          </button>
        </div>
      </div>
    </div>
  {/if}

  <div class="stea-tabs" role="tablist" aria-label="Investlog views">
    <button
      class={activeTab === 'assets' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'assets'}
      on:click={() => selectActiveTab('assets')}
    >
      Assets
    </button>
    <button
      class={activeTab === 'analysis' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'analysis'}
      on:click={() => selectActiveTab('analysis')}
    >
      Analysis
    </button>
    <button
      class={activeTab === 'screener' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'screener'}
      on:click={() => selectActiveTab('screener')}
    >
      Screener
    </button>
    <button
      class={activeTab === 'watchlist' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'watchlist'}
      on:click={() => selectActiveTab('watchlist')}
    >
      Watchlist
    </button>
    <button
      class={activeTab === 'history' ? 'stea-tab stea-tab-active' : 'stea-tab'}
      type="button"
      role="tab"
      aria-selected={activeTab === 'history'}
      on:click={() => selectActiveTab('history')}
    >
      History
    </button>
    {#if activeTab === 'assets'}
      <button class="stea-tab-action" type="button" aria-label="Add entry" on:click={openModal}>
        <Plus size={16} />
      </button>
    {/if}
  </div>

  {#if activeTab === 'assets'}
    <section class="stea-stack" aria-label="Current investment assets">
      {#if isLoading}
        <p class="stea-muted">Loading assets</p>
      {:else}
        {#if assetsSummary}
          <dl class="stea-stat-grid stea-stat-strip stea-assets-summary">
            <div>
              <dt>Total buys</dt>
              <dd>{formatMoney(assetsSummary.total_buys)}</dd>
            </div>
            <div>
              <dt>Total sells</dt>
              <dd>{formatMoney(assetsSummary.total_sells)}</dd>
            </div>
            <div>
              <dt>Commissions</dt>
              <dd>{formatMoney(assetsSummary.total_commissions)}</dd>
            </div>
            <div>
              <dt>Realized</dt>
              <dd class={changeClass(assetsSummary.realized_profit)}>{formatMoney(assetsSummary.realized_profit)}</dd>
            </div>
            <div>
              <dt>Unrealized</dt>
              <dd class={changeClass(assetsSummary.unrealized_profit)}>{formatMoney(assetsSummary.unrealized_profit)}</dd>
            </div>
            <div>
              <dt>Net profit</dt>
              <dd class={changeClass(assetsSummary.net_profit)}>{formatMoney(assetsSummary.net_profit)}</dd>
            </div>
          </dl>
        {/if}

        {#if assets.length === 0}
          <p class="stea-muted">No current assets.</p>
        {:else}
        <div class="stea-table-wrap">
          <table class="stea-table">
            <thead>
              <tr>
                <th>Ticker</th>
                <th>Days</th>
                <th>%</th>
                <th>Change</th>
                <th>Cost</th>
                <th>Qty</th>
                <th>Price</th>
                <th>Current</th>
                <th>Price change</th>
                <th>Watchlist</th>
              </tr>
            </thead>
            <tbody>
              {#each assets as asset}
                <tr>
                  <td>
                    <button class="stea-ticker-trigger" type="button" on:click={() => handleTickerProfileTap(asset.ticker)}>
                      {asset.ticker}
                    </button>
                  </td>
                  <td>{formatDays(asset.days_since_buy_midpoint)}</td>
                  <td class={changeClass(asset.percent_change)}>{formatPercent(asset.percent_change)}</td>
                  <td class={changeClass(asset.amount_change)}>{formatMoney(asset.amount_change)}</td>
                  <td>{formatMoney(asset.cost)}</td>
                  <td>{formatQuantity(asset.quantity)}</td>
                  <td>{formatMoney(asset.avg_buy_price)}</td>
                  <td>{formatMoney(asset.current_price)}</td>
                  <td class={changeClass(asset.price_change)}>{formatMoney(asset.price_change)}</td>
                  <td>
                    <button
                      class="stea-icon-btn stea-icon-btn-sm"
                      type="button"
                      aria-label={watchlistActionLabel(asset.ticker)}
                      title={watchlistActionLabel(asset.ticker)}
                      on:click={() => toggleWatchlist(asset.ticker)}
                    >
                      {#if activeWatchlistTickers.includes(asset.ticker)}
                        <StarOff size={16} />
                      {:else}
                        <Star size={16} />
                      {/if}
                    </button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {/if}
      {/if}
    </section>
  {:else if activeTab === 'analysis'}
    <section class="stea-stack" aria-label="Ticker analysis against benchmark">
      <TickerPicker
        label=""
        selected={selectedAnalysisTickers}
        assetTickers={assetTickers}
        watchlistTickers={activeWatchlistTickers}
        history={recentAnalysisTickers}
        search={searchTickers}
        onSelect={addAnalysisTicker}
        onRemove={removeAnalysisTicker}
        onHistoryChange={setAnalysisTickerHistory}
        onProfile={openCompanyProfile}
      />

      <section class="stea-picker-section" aria-label="Analysis range">
        <div class="stea-row-between stea-analysis-range-row">
          <div>
            <p class="stea-picker-heading">Range</p>
            <div class="stea-chip-row">
              {#each performanceRanges as range}
                <button
                  class="stea-chip"
                  class:stea-chip-active={performanceRange === range.value}
                  type="button"
                  aria-pressed={performanceRange === range.value}
                  on:click={() => selectPerformanceRange(range.value)}
                >
                  {range.label}
                </button>
              {/each}
            </div>
          </div>
          <div class="stea-analysis-range-actions">
            {#if selectedAnalysisTickers.some((ticker) => !activeWatchlistTickers.includes(ticker))}
              <button
                class="stea-icon-btn stea-icon-btn-sm"
                type="button"
                aria-label="Add selected tickers to watchlist"
                title="Add selected to watchlist"
                on:click={addSelectedToWatchlist}
              >
                <Star size={16} />
              </button>
            {/if}
          </div>
        </div>
      </section>

      {#if isLoadingPerformance}
        <p class="stea-muted">Loading performance</p>
      {:else if performance}
        <PerformanceChart
          {performance}
          assetTickers={assetTickers}
          colorTickers={analysisColorTickers}
          interval={analysisInterval}
          onIntervalChange={setAnalysisInterval}
        />
      {:else}
        <p class="stea-muted">Add a ticker to start analysis.</p>
      {/if}

      {#if selectedTickerNotesForAnalysis.length > 0}
        <section class="stea-stack" aria-label="Selected ticker notes">
          <p class="stea-picker-heading">Ticker notes</p>
          <div class="stea-analysis-notes">
            {#each selectedTickerNotesForAnalysis as item}
              <article class="stea-panel-grid">
                <div class="stea-row-between">
                  <button class="stea-ticker-trigger" type="button" on:click={() => openTickerNotes(item)}>
                    {item.ticker}
                  </button>
                  <div class="stea-row">
                    <span class="stea-note">{formatNoteCount(item.notes.length)}</span>
                    <button
                      class="stea-icon-btn stea-icon-btn-sm"
                      type="button"
                      aria-label={`Add note for ${item.ticker}`}
                      title="Add note"
                      on:click={() => addTickerNote(item.ticker)}
                    >
                      <MessageSquare size={16} />
                    </button>
                  </div>
                </div>
                {#if item.notes.length === 0}
                  <p class="stea-muted">No notes yet.</p>
                {:else}
                  <div class="stea-watchlist-notes">
                    {#each item.notes as note}
                      <article class="stea-watchlist-note">
                        <p>{note.note}</p>
                        <span>{formatDate(note.created_at)}</span>
                      </article>
                    {/each}
                  </div>
                {/if}
              </article>
            {/each}
          </div>
        </section>
      {/if}
    </section>
  {:else if activeTab === 'screener'}
    <AiCorrectionsPage
      watchlistTickers={activeWatchlistTickers}
      onAddToWatchlist={addToWatchlist}
      onRemoveFromWatchlist={removeFromWatchlist}
    />
  {:else if activeTab === 'watchlist'}
    <section class="stea-stack" aria-label="Investment watchlist">
      {#if watchlist.length === 0}
        <p class="stea-muted">No watchlist tickers.</p>
      {:else}
        <div class="stea-table-wrap">
          <table class="stea-table stea-table-readable stea-watchlist-table">
            <thead>
              <tr>
                <th>Ticker</th>
                <th>Company</th>
                <th>Price</th>
                <th>Cap</th>
                <th>Shares</th>
                <th>Debt</th>
                <th>Revenue</th>
                <th>Cash</th>
                <th>FCF</th>
                <th>Description</th>
                <th>Status</th>
                <th>Notes</th>
                <th>Meta</th>
                <th>Price updated</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {#each watchlist as item}
                <tr>
                  <td>
                    <button class="stea-ticker-trigger" type="button" on:click={() => openTickerNotes(watchlistTickerNotesView(item))}>
                      {item.ticker}
                    </button>
                  </td>
                  <td><span class="stea-cell-clip">{item.company_name ?? '-'}</span></td>
                  <td>{formatOptionalMoney(item.current_price)}</td>
                  <td>{formatCompactMoney(item.market_cap)}</td>
                  <td>{formatShares(item.shares_outstanding)}</td>
                  <td>{formatCompactMoney(item.total_debt)}</td>
                  <td>{formatCompactMoney(item.revenue)}</td>
                  <td>{formatCompactMoney(item.cash)}</td>
                  <td>{formatCompactMoney(item.free_cash_flow)}</td>
                  <td><span class="stea-cell-clip">{item.description ?? '-'}</span></td>
                  <td>{item.is_active ? 'Active' : 'Removed'}</td>
                  <td>
                    <button
                      class="stea-icon-btn stea-icon-btn-sm"
                      type="button"
                      aria-label={`Show ${item.ticker} ticker notes`}
                      title={formatNoteCount(item.notes.length)}
                      on:click={() => openTickerNotes(watchlistTickerNotesView(item))}
                    >
                      <MessageSquare size={16} />
                    </button>
                  </td>
                  <td>{formatDate(item.meta_fetched_at)}</td>
                  <td>{formatDate(item.price_fetched_at)}</td>
                  <td>
                    {#if item.is_active}
                      <button
                        class="stea-icon-btn stea-icon-btn-sm"
                        type="button"
                        aria-label={`Remove ${item.ticker} from watchlist`}
                        title="Remove from watchlist"
                        on:click={() => removeFromWatchlist(item.ticker)}
                      >
                        <StarOff size={16} />
                      </button>
                    {:else}
                      <button
                        class="stea-icon-btn stea-icon-btn-sm"
                        type="button"
                        aria-label={`Re-add ${item.ticker} to watchlist`}
                        title="Re-add to watchlist"
                        on:click={() => addToWatchlist(item.ticker)}
                      >
                        <Star size={16} />
                      </button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>
  {:else if activeTab === 'history'}
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
              <button class="stea-ticker-trigger" type="button" on:click={() => handleTickerProfileTap(entry.ticker)}>
                {entry.ticker}
              </button>
              <div class="stea-row">
                <button
                  class="stea-icon-btn stea-icon-btn-sm"
                  type="button"
                  aria-label={`Edit ${entry.ticker} entry`}
                  title="Edit entry"
                  on:click={() => openEditEntry(entry)}
                >
                  <Pencil size={16} />
                </button>
                <button
                  class="stea-icon-btn stea-icon-btn-sm"
                  type="button"
                  aria-label={watchlistActionLabel(entry.ticker)}
                  title={watchlistActionLabel(entry.ticker)}
                  on:click={() => toggleWatchlist(entry.ticker)}
                >
                  {#if activeWatchlistTickers.includes(entry.ticker)}
                    <StarOff size={16} />
                  {:else}
                    <Star size={16} />
                  {/if}
                </button>
                <span class={entry.op === 'buy' ? 'stea-badge stea-badge-buy' : 'stea-badge stea-badge-sell'}>{entry.op}</span>
              </div>
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
