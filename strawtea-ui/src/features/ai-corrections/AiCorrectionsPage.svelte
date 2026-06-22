<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { RefreshCw, Search, SlidersHorizontal, Star, StarOff } from '@lucide/svelte';
  import {
    fetchLatestAiCorrectionScreenerRun,
    fetchInvestlogPerformance,
    abortJob,
    stopJob,
    startAiCorrectionScreenerRun,
    updateAiCorrectionOverride,
    type AiScreenerResult,
    type AiScreenerRun,
    type AiScreenerStatus,
    type InvestlogPerformance,
    type InvestlogPerformanceRange
  } from '../../lib/api';
  import type { AnalysisIntervalSetting } from '../../lib/settings';
  import PerformanceChart from '../investlog/PerformanceChart.svelte';

  const statusOptions: Array<'all' | AiScreenerStatus> = [
    'all',
    'Entry Candidate',
    'Watch',
    'Rejected',
    'Ignore'
  ];
  const tierOptions = ['all', '1', '2', '3', 'none'];
  const performanceRanges: InvestlogPerformanceRange[] = ['1m', '3m', '6m', '1y', '3y'];

  export let watchlistTickers: string[] = [];
  export let onAddToWatchlist: (ticker: string) => void | Promise<void> = () => {};
  export let onRemoveFromWatchlist: (ticker: string) => void | Promise<void> = () => {};

  let run: AiScreenerRun | null = null;
  let selectedTicker = '';
  let statusFilter: 'all' | AiScreenerStatus = 'all';
  let tierFilter = 'all';
  let minScore = 0;
  let search = '';
  let isLoading = true;
  let isStarting = false;
  let isControllingJob = false;
  let isSaving = false;
  let error = '';
  let pollTimer: number | undefined;
  let overrideTier = '';
  let overrideScore = '';
  let overrideStatus: '' | AiScreenerStatus = '';
  let overrideNotes = '';
  let modalPerformance: InvestlogPerformance | null = null;
  let isLoadingModalPerformance = false;
  let modalError = '';
  let performanceRange: InvestlogPerformanceRange = '1y';
  let analysisInterval: AnalysisIntervalSetting | null = null;

  $: isActiveRun =
    run?.status === 'queued' ||
    run?.status === 'running' ||
    run?.status === 'waiting_rate_limit';
  $: canStopRun =
    !!run?.job_id &&
    (run.status === 'queued' || run.status === 'running' || run.status === 'waiting_rate_limit');
  $: canResumeRun = !!run?.job_id && (run.status === 'stopped' || run.status === 'waiting_rate_limit');
  $: canAbortRun =
    !!run?.job_id &&
    ['queued', 'running', 'waiting_rate_limit', 'stopped', 'failed'].includes(run.status);
  $: progressText = run ? `${run.processed_count}/${run.universe_count || 0}` : '0/0';
  $: budgetText = run ? `${run.twelve_budget_used}/${run.twelve_budget_limit}` : '0/0';
  $: selected = run?.results.find((result) => result.ticker === selectedTicker) ?? null;
  $: statusCounts = countResultsByStatus(run?.results ?? []);
  $: filteredResults = (run?.results ?? []).filter((result) => {
    const matchesStatus = statusFilter === 'all' || result.status === statusFilter;
    const matchesTier =
      tierFilter === 'all' ||
      (tierFilter === 'none' ? !result.ai_tier : result.ai_tier === tierFilter);
    const matchesScore = result.ai_score >= minScore;
    const q = search.trim().toUpperCase();
    const matchesSearch =
      !q ||
      result.ticker.includes(q) ||
      result.company_name.toUpperCase().includes(q);

    return matchesStatus && matchesTier && matchesScore && matchesSearch;
  });
  $: groupedResults = groupResultsByRun(filteredResults);

  onMount(async () => {
    await loadLatest();
  });

  onDestroy(() => {
    stopPolling();
  });

  async function loadLatest() {
    isLoading = true;
    error = '';
    try {
      setRun(await fetchLatestAiCorrectionScreenerRun());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load screener run';
    } finally {
      isLoading = false;
    }
  }

  async function startRun() {
    isStarting = true;
    error = '';
    try {
      setRun(await startAiCorrectionScreenerRun());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not start screener run';
    } finally {
      isStarting = false;
    }
  }

  async function stopRun() {
    if (!run?.job_id) return;
    isControllingJob = true;
    error = '';
    try {
      await stopJob(run.job_id);
      setRun(await fetchLatestAiCorrectionScreenerRun());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not stop job';
    } finally {
      isControllingJob = false;
    }
  }

  async function resumeRun() {
    await startRun();
  }

  async function abortRun() {
    if (!run?.job_id) return;
    isControllingJob = true;
    error = '';
    try {
      await abortJob(run.job_id);
      setRun(await fetchLatestAiCorrectionScreenerRun());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not abort job';
    } finally {
      isControllingJob = false;
    }
  }

  async function pollRun() {
    try {
      setRun(await fetchLatestAiCorrectionScreenerRun());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not refresh screener run';
      stopPolling();
    }
  }

  function setRun(nextRun: AiScreenerRun | null) {
    run = nextRun;
    if (selectedTicker && !run?.results.some((result) => result.ticker === selectedTicker)) {
      closeModal();
    }
    if (run?.status === 'queued' || run?.status === 'running' || run?.status === 'waiting_rate_limit') {
      startPolling();
    } else {
      stopPolling();
    }
  }

  function startPolling() {
    if (pollTimer) return;
    pollTimer = window.setInterval(pollRun, 1000);
  }

  function stopPolling() {
    if (!pollTimer) return;
    window.clearInterval(pollTimer);
    pollTimer = undefined;
  }

  async function openResult(result: AiScreenerResult) {
    selectedTicker = result.ticker;
    overrideTier = result.manual_ai_tier ?? '';
    overrideScore = result.manual_ai_score == null ? '' : String(result.manual_ai_score);
    overrideStatus = result.manual_status ?? '';
    overrideNotes = result.manual_notes;
    modalPerformance = null;
    modalError = '';
    analysisInterval = null;
    await loadModalPerformance(result.ticker);
  }

  function closeModal() {
    selectedTicker = '';
    modalPerformance = null;
    modalError = '';
    analysisInterval = null;
  }

  async function loadModalPerformance(ticker: string) {
    isLoadingModalPerformance = true;
    modalError = '';
    try {
      modalPerformance = await fetchInvestlogPerformance([ticker], performanceRange);
    } catch (err) {
      modalError = err instanceof Error ? err.message : 'Could not load chart analysis';
    } finally {
      isLoadingModalPerformance = false;
    }
  }

  async function selectPerformanceRange(range: InvestlogPerformanceRange) {
    performanceRange = range;
    analysisInterval = null;
    if (selectedTicker) {
      await loadModalPerformance(selectedTicker);
    }
  }

  function setAnalysisInterval(interval: AnalysisIntervalSetting | null) {
    analysisInterval = interval;
  }

  async function saveOverride() {
    if (!selected) return;

    const parsedScore = overrideScore.trim() === '' ? null : Number(overrideScore);
    if (parsedScore != null && (!Number.isFinite(parsedScore) || parsedScore < 0 || parsedScore > 100)) {
      error = 'Manual score must be between 0 and 100';
      return;
    }

    isSaving = true;
    error = '';
    try {
      setRun(
        await updateAiCorrectionOverride(selected.ticker, {
          manual_ai_tier: overrideTier || null,
          manual_ai_score: parsedScore,
          manual_status: overrideStatus || null,
          notes: overrideNotes
        })
      );
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not save override';
    } finally {
      isSaving = false;
    }
  }

  function formatDate(value: string | null | undefined) {
    if (!value) return 'Never';
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(new Date(value));
  }

  function formatMoney(value: number | null) {
    if (value == null) return '-';
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 2
    }).format(value);
  }

  function formatPercent(value: number | null) {
    if (value == null) return '-';
    return `${value.toFixed(1)}%`;
  }

  function tierLabel(value: string | null) {
    if (value === '1') return 'Tier 1';
    if (value === '2') return 'Tier 2';
    if (value === '3') return 'Tier 3';
    return 'Unassigned';
  }

  function statusClass(value: string) {
    return value.toLowerCase().replace(/\s+/g, '-');
  }

  function isWatchlisted(ticker: string) {
    return watchlistTickers.includes(ticker);
  }

  function watchlistActionLabel(ticker: string) {
    return isWatchlisted(ticker) ? 'Remove watchlist' : 'Add watchlist';
  }

  function toggleWatchlist(ticker: string) {
    if (isWatchlisted(ticker)) {
      onRemoveFromWatchlist(ticker);
      return;
    }

    onAddToWatchlist(ticker);
  }

  function countResultsByStatus(results: AiScreenerResult[]) {
    return statusOptions
      .filter((status): status is AiScreenerStatus => status !== 'all')
      .map((status) => ({
        status,
        count: results.filter((result) => result.status === status).length
      }));
  }

  function groupResultsByRun(results: AiScreenerResult[]) {
    const groups: Array<{
      runId: string;
      completedAt: string | null;
      results: AiScreenerResult[];
    }> = [];

    for (const result of results) {
      let group = groups.find((item) => item.runId === result.run_id);
      if (!group) {
        group = {
          runId: result.run_id,
          completedAt: result.run_completed_at,
          results: []
        };
        groups.push(group);
      }
      group.results.push(result);
    }

    return groups;
  }

  function runCaption(completedAt: string | null) {
    return `Run finished ${formatDate(completedAt)}`;
  }
</script>

<section class="stea-stack-lg">
  <div class="stea-row-between stea-screener-header">
    <div>
      <p class="stea-eyebrow">Latest run</p>
      <p class="stea-text">{run ? `${run.status} · ${formatDate(run.completed_at ?? run.started_at ?? run.created_at)}` : 'Never'}</p>
      {#if run?.status_reason}
        <p class="stea-muted">{run.status_reason}{run.run_after ? ` · next check ${formatDate(run.run_after)}` : ''}</p>
      {/if}
      {#if run?.latest_event}
        <p class="stea-note">{run.latest_event.message} · {formatDate(run.latest_event.created_at)}</p>
      {/if}
      {#if run}
        <p class="stea-muted">Twelve budget {budgetText} credits</p>
      {/if}
    </div>
    <div class="stea-row stea-screener-actions">
      {#if canStopRun}
        <button class="stea-btn-secondary stea-btn-fit" type="button" disabled={isControllingJob} on:click={stopRun}>
          Stop
        </button>
      {/if}
      {#if canAbortRun}
        <button class="stea-btn-secondary stea-btn-fit" type="button" disabled={isControllingJob} on:click={abortRun}>
          Abort
        </button>
      {/if}
      <button class="stea-btn-primary stea-btn-fit" type="button" disabled={isStarting || (isActiveRun && run?.status !== 'waiting_rate_limit')} on:click={canResumeRun ? resumeRun : startRun}>
        <RefreshCw size={18} />
        {run?.status === 'stopped' || run?.status === 'waiting_rate_limit' ? 'Resume screener' : isActiveRun ? `Running ${progressText}` : 'Run screener'}
      </button>
    </div>
  </div>

  {#if error}
    <p class="stea-error">{error}</p>
  {/if}

  {#if run?.status === 'failed'}
    <p class="stea-error">{run.error}</p>
  {/if}

  <section class="stea-form-grid stea-screener-filters">
    <label class="stea-field">
      <span class="stea-field-label">Search</span>
      <span class="stea-search">
        <Search size={18} />
        <input bind:value={search} type="search" placeholder="Ticker or company" />
      </span>
    </label>
    <label class="stea-field">
      <span class="stea-field-label">Status</span>
      <select class="stea-input" bind:value={statusFilter}>
        {#each statusOptions as option}
          <option value={option}>{option === 'all' ? 'All statuses' : option}</option>
        {/each}
      </select>
    </label>
    <label class="stea-field">
      <span class="stea-field-label">AI tier</span>
      <select class="stea-input" bind:value={tierFilter}>
        {#each tierOptions as option}
          <option value={option}>{option === 'all' ? 'All tiers' : option === 'none' ? 'Unassigned' : tierLabel(option)}</option>
        {/each}
      </select>
    </label>
    <label class="stea-field">
      <span class="stea-field-label">Minimum AI score</span>
      <input class="stea-input" bind:value={minScore} min="0" max="100" type="number" />
    </label>
  </section>
  {#if run}
    <div class="stea-status-summary" aria-label="Processed tickers by status">
      <span class="stea-muted">Processed {run.results.length}</span>
      {#each statusCounts as item}
        <span class={`stea-status stea-status-${statusClass(item.status)}`}>{item.status}: {item.count}</span>
      {/each}
    </div>
  {/if}

  {#if isLoading}
    <p class="stea-muted">Loading</p>
  {:else if !run}
    <section class="stea-empty">
      <div>
        <SlidersHorizontal size={38} />
        <h2>No screener run yet</h2>
      </div>
    </section>
  {:else}
      <div class="stea-table-wrap">
        <table class="stea-table stea-table-readable stea-screener-table">
          <thead>
            <tr>
              <th>Ticker</th>
              <th>Company</th>
              <th>Tier</th>
              <th>Score</th>
              <th>Status</th>
              <th>Price</th>
              <th>Correction</th>
              <th>Trend dist.</th>
              <th>Momentum</th>
              <th>Volume</th>
              <th>Reject reason</th>
              <th>Processed</th>
              <th>Watchlist</th>
            </tr>
          </thead>
          <tbody>
            {#if filteredResults.length === 0}
              <tr>
                <td class="stea-table-empty" colspan="13">No matching tickers</td>
              </tr>
            {:else}
              {#each groupedResults as group}
                <tr class="stea-run-caption-row">
                  <td colspan="13">{runCaption(group.completedAt)}</td>
                </tr>
                {#each group.results as result}
                  <tr class:selected={selectedTicker === result.ticker}>
                    <td>
                      <button class="stea-ticker-trigger" type="button" on:click={() => openResult(result)}>
                        {result.ticker}
                      </button>
                    </td>
                    <td>{result.company_name}</td>
                    <td>{tierLabel(result.ai_tier)}</td>
                    <td>{result.ai_score}</td>
                    <td><span class={`stea-status stea-status-${statusClass(result.status)}`}>{result.status}</span></td>
                    <td>{formatMoney(result.current_price)}</td>
                    <td>{formatPercent(result.correction_depth)}</td>
                    <td>{formatPercent(result.trend_distance)}</td>
                    <td>{result.momentum_condition}</td>
                    <td>{result.volume_condition}</td>
                    <td>{result.rejection_reason ?? '-'}</td>
                    <td>{formatDate(result.processed_at)}</td>
                    <td>
                      <button
                        class="stea-icon-btn stea-icon-btn-sm"
                        type="button"
                        aria-label={watchlistActionLabel(result.ticker)}
                        title={watchlistActionLabel(result.ticker)}
                        on:click={() => toggleWatchlist(result.ticker)}
                      >
                        {#if isWatchlisted(result.ticker)}
                          <StarOff size={16} />
                        {:else}
                          <Star size={16} />
                        {/if}
                      </button>
                    </td>
                  </tr>
                {/each}
              {/each}
            {/if}
          </tbody>
        </table>
      </div>

      {#if selected}
        <div class="stea-modal-backdrop" role="presentation" on:pointerdown={closeModal}>
        <div class="stea-modal stea-analysis-modal" role="dialog" aria-modal="true" aria-labelledby="ai-correction-detail-title" tabindex="-1" on:pointerdown|stopPropagation>
          <div class="stea-modal-header">
            <div>
              <p class="stea-eyebrow">{selected.status}</p>
              <h2 id="ai-correction-detail-title" class="stea-heading-sm">{selected.ticker} · {selected.company_name}</h2>
            </div>
            <div class="stea-row">
              <button
                class="stea-icon-btn stea-icon-btn-sm"
                type="button"
                aria-label={watchlistActionLabel(selected.ticker)}
                title={watchlistActionLabel(selected.ticker)}
                on:click={() => toggleWatchlist(selected.ticker)}
              >
                {#if isWatchlisted(selected.ticker)}
                  <StarOff size={16} />
                {:else}
                  <Star size={16} />
                {/if}
              </button>
              <button class="stea-icon-btn" type="button" aria-label="Close" on:click={closeModal}>×</button>
            </div>
          </div>

          <section class="stea-stack">
            <div class="stea-row-between stea-analysis-modal-toolbar">
              <h3 class="stea-heading-sm">Chart analyzer</h3>
              <div class="stea-chip-row">
                {#each performanceRanges as range}
                  <button
                    class="stea-chip"
                    class:stea-chip-active={performanceRange === range}
                    type="button"
                    on:click={() => selectPerformanceRange(range)}
                  >
                    {range}
                  </button>
                {/each}
              </div>
            </div>

            {#if isLoadingModalPerformance}
              <p class="stea-muted">Loading chart analysis</p>
            {:else if modalError}
              <p class="stea-error">{modalError}</p>
            {:else if modalPerformance}
              <PerformanceChart
                performance={modalPerformance}
                assetTickers={[selected.ticker]}
                colorTickers={[selected.ticker]}
                interval={analysisInterval}
                onIntervalChange={setAnalysisInterval}
              />
            {/if}
          </section>

          <section class="stea-screener-detail">
          <div>
            <p class="stea-eyebrow">{selected.status}</p>
            <h2 class="stea-heading-sm">{selected.ticker} · {selected.company_name}</h2>
          </div>

          <dl class="stea-stat-grid stea-profile-grid">
            <div>
              <dt>AI tier</dt>
              <dd>{tierLabel(selected.ai_tier)}</dd>
            </div>
            <div>
              <dt>AI score</dt>
              <dd>{selected.ai_score}</dd>
            </div>
            <div>
              <dt>Correction</dt>
              <dd>{formatPercent(selected.correction_depth)}</dd>
            </div>
            <div>
              <dt>Trend distance</dt>
              <dd>{formatPercent(selected.trend_distance)}</dd>
            </div>
            <div>
              <dt>Recovery from low</dt>
              <dd>{formatPercent(selected.rationale.technical_metrics?.recovery_from_low ?? null)}</dd>
            </div>
            <div>
              <dt>Days since low</dt>
              <dd>{selected.rationale.technical_metrics?.days_since_low ?? '-'}</dd>
            </div>
          </dl>

          <section class="stea-stack">
            <p class="stea-note">{selected.rationale.company_summary}</p>
            <p class="stea-note">{selected.rationale.technical}</p>
            {#if selected.rejection_reason}
              <p class="stea-error">{selected.rejection_reason}</p>
            {/if}
          </section>

          <section class="stea-stack">
            <h3 class="stea-heading-sm">AI rationale</h3>
            <div class="stea-chip-row">
              {#each selected.rationale.ai?.theme_matches ?? [] as theme}
                <span class="stea-chip">{theme}</span>
              {/each}
            </div>
            <ul class="stea-detail-list">
              {#each selected.rationale.ai?.reasons ?? [] as reason}
                <li>{reason}</li>
              {/each}
            </ul>
            {#if selected.rationale.ai?.warnings?.length}
              <ul class="stea-detail-list stea-muted">
                {#each selected.rationale.ai.warnings as warning}
                  <li>{warning}</li>
                {/each}
              </ul>
            {/if}
          </section>

          <section class="stea-form-grid stea-override-form">
            <label class="stea-field">
              <span class="stea-field-label">Manual tier</span>
              <select class="stea-input" bind:value={overrideTier}>
                <option value="">No override</option>
                <option value="1">Tier 1</option>
                <option value="2">Tier 2</option>
                <option value="3">Tier 3</option>
              </select>
            </label>
            <label class="stea-field">
              <span class="stea-field-label">Manual score</span>
              <input class="stea-input" bind:value={overrideScore} min="0" max="100" type="number" placeholder="No override" />
            </label>
            <label class="stea-field">
              <span class="stea-field-label">Manual status</span>
              <select class="stea-input" bind:value={overrideStatus}>
                <option value="">No override</option>
                <option value="Entry Candidate">Entry Candidate</option>
                <option value="Watch">Watch</option>
                <option value="Rejected">Rejected</option>
                <option value="Ignore">Ignore</option>
              </select>
            </label>
            <label class="stea-field stea-span-all">
              <span class="stea-field-label">Notes</span>
              <textarea class="stea-input stea-textarea" bind:value={overrideNotes} rows="3"></textarea>
            </label>
            <div class="stea-span-all stea-row-between">
              <span class="stea-muted">{selected.manual_ai_tier || selected.manual_ai_score != null || selected.manual_status ? 'Override applied' : 'No override applied'}</span>
              <button class="stea-btn-secondary stea-btn-fit" type="button" disabled={isSaving} on:click={saveOverride}>
                Save override
              </button>
            </div>
          </section>
          </section>
        </div>
        </div>
      {/if}
  {/if}
</section>
