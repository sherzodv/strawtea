<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, FileUp } from '@lucide/svelte';
  import {
    confirmRawtxImport,
    listCategorizationPatterns,
    listMonthlySpends,
    listRawtx,
    previewRawtxImport,
    type RawtxCategorizationPattern,
    type RawtxImportPreview,
    type RawtxMonthlySpend,
    type RawtxPreviewRow,
    type RawtxRow
  } from '../../lib/api';

  type Tab = 'monthly' | 'categorize' | 'rawtx' | 'import';

  let activeTab: Tab = 'monthly';
  let selectedFile: File | null = null;
  let preview: RawtxImportPreview | null = null;
  let isUploading = false;
  let isConfirming = false;
  let isLoadingCategorization = false;
  let isLoadingMonthly = false;
  let isLoadingRawtx = false;
  let error = '';
  let categorizationError = '';
  let monthlyError = '';
  let rawtxError = '';
  let categorizationPatterns: RawtxCategorizationPattern[] = [];
  let monthlyRows: RawtxMonthlySpend[] = [];
  let rawtxRows: RawtxRow[] = [];
  let rawtxTotal = 0;
  let rawtxQuery = '';
  let rawtxOffset = 0;
  const rawtxLimit = 100;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  $: duplicateRows = preview?.rows.filter((row) => row.is_duplicate).length ?? 0;
  $: newRows = preview ? preview.rows.length - duplicateRows : 0;
  $: rawtxStart = rawtxTotal === 0 ? 0 : rawtxOffset + 1;
  $: rawtxEnd = Math.min(rawtxOffset + rawtxRows.length, rawtxTotal);

  onMount(() => {
    loadCategorizationPatterns();
    loadMonthlySpends();
    loadRawtx();
  });

  function onFileChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    selectedFile = input.files?.[0] ?? null;
    preview = null;
    error = '';
  }

  async function uploadPreview() {
    if (!selectedFile) return;
    isUploading = true;
    error = '';

    try {
      preview = await previewRawtxImport(selectedFile);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Import preview failed';
    } finally {
      isUploading = false;
    }
  }

  async function confirmImport() {
    if (!preview) return;
    isConfirming = true;
    error = '';

    try {
      preview = await confirmRawtxImport(preview.import.id);
      activeTab = 'monthly';
      rawtxOffset = 0;
      await loadCategorizationPatterns();
      await loadMonthlySpends();
      await loadRawtx();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Import confirm failed';
    } finally {
      isConfirming = false;
    }
  }

  async function loadCategorizationPatterns() {
    isLoadingCategorization = true;
    categorizationError = '';

    try {
      categorizationPatterns = await listCategorizationPatterns();
    } catch (err) {
      categorizationError =
        err instanceof Error ? err.message : 'Could not load categorization patterns';
    } finally {
      isLoadingCategorization = false;
    }
  }

  async function loadMonthlySpends() {
    isLoadingMonthly = true;
    monthlyError = '';

    try {
      monthlyRows = await listMonthlySpends();
    } catch (err) {
      monthlyError = err instanceof Error ? err.message : 'Could not load monthly spends';
    } finally {
      isLoadingMonthly = false;
    }
  }

  async function loadRawtx() {
    isLoadingRawtx = true;
    rawtxError = '';

    try {
      const result = await listRawtx({
        q: rawtxQuery,
        limit: rawtxLimit,
        offset: rawtxOffset
      });
      rawtxRows = result.rows;
      rawtxTotal = result.total;
      rawtxOffset = result.offset;
    } catch (err) {
      rawtxError = err instanceof Error ? err.message : 'Could not load raw transactions';
    } finally {
      isLoadingRawtx = false;
    }
  }

  function scheduleRawtxSearch() {
    rawtxOffset = 0;
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      loadRawtx();
    }, 250);
  }

  async function previousRawtxPage() {
    rawtxOffset = Math.max(0, rawtxOffset - rawtxLimit);
    await loadRawtx();
  }

  async function nextRawtxPage() {
    if (rawtxOffset + rawtxLimit >= rawtxTotal) return;
    rawtxOffset += rawtxLimit;
    await loadRawtx();
  }

  function money(amount: number, currency: string) {
    const sign = amount < 0 ? '-' : '';
    return `${sign}${(Math.abs(amount) / 100).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    })} ${currency}`;
  }

  function dateTime(value: string) {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(new Date(value));
  }

  function monthLabel(value: string) {
    return new Intl.DateTimeFormat(undefined, {
      month: 'long',
      year: 'numeric'
    }).format(new Date(`${value}T00:00:00`));
  }

  function kindLabel(row: RawtxPreviewRow | RawtxRow) {
    return row.raw_kind ?? row.direction;
  }
</script>

<section class="stea-stack-lg">
  <div class="stea-tabs" role="tablist" aria-label="Spends sections">
    <button
      class:stea-tab-active={activeTab === 'monthly'}
      class="stea-tab"
      type="button"
      on:click={() => (activeTab = 'monthly')}
    >
      Monthly
    </button>
    <button
      class:stea-tab-active={activeTab === 'categorize'}
      class="stea-tab"
      type="button"
      on:click={() => (activeTab = 'categorize')}
    >
      Categorize
    </button>
    <button
      class:stea-tab-active={activeTab === 'rawtx'}
      class="stea-tab"
      type="button"
      on:click={() => (activeTab = 'rawtx')}
    >
      Rawtx
    </button>
    <button
      class:stea-tab-active={activeTab === 'import'}
      class="stea-tab"
      type="button"
      on:click={() => (activeTab = 'import')}
    >
      Import
    </button>
  </div>

  {#if activeTab === 'monthly'}
    <section class="stea-stack">
      <div class="stea-panel-grid">
        <div class="stea-row-between">
          <div>
            <h2 class="stea-heading-sm">Monthly spends</h2>
            <p class="stea-text">
              {#if isLoadingMonthly}
                Loading
              {:else}
                {monthlyRows.length} grouped totals
              {/if}
            </p>
          </div>
          <button
            class="stea-btn-secondary stea-btn-fit"
            type="button"
            disabled={isLoadingMonthly}
            on:click={loadMonthlySpends}
          >
            Refresh
          </button>
        </div>

        {#if monthlyError}
          <p class="stea-error">{monthlyError}</p>
        {/if}
      </div>

      <div class="stea-table-wrap">
        <table class="stea-table stea-table-readable">
          <thead>
            <tr>
              <th>Month</th>
              <th>Currency</th>
              <th>Spend</th>
              <th>Transactions</th>
            </tr>
          </thead>
          <tbody>
            {#each monthlyRows as row}
              <tr>
                <td>{monthLabel(row.month)}</td>
                <td>{row.currency}</td>
                <td>{money(row.amount, row.currency)}</td>
                <td>{row.transaction_count}</td>
              </tr>
            {:else}
              <tr>
                <td colspan="4" class="stea-table-empty">No monthly spends</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}

  {#if activeTab === 'categorize'}
    <section class="stea-stack">
      <div class="stea-panel-grid">
        <div class="stea-row-between">
          <div>
            <h2 class="stea-heading-sm">QR patterns</h2>
            <p class="stea-text">
              {#if isLoadingCategorization}
                Loading
              {:else}
                {categorizationPatterns.length} distinct patterns
              {/if}
            </p>
          </div>
          <button
            class="stea-btn-secondary stea-btn-fit"
            type="button"
            disabled={isLoadingCategorization}
            on:click={loadCategorizationPatterns}
          >
            Refresh
          </button>
        </div>

        {#if categorizationError}
          <p class="stea-error">{categorizationError}</p>
        {/if}
      </div>

      <div class="stea-table-wrap">
        <table class="stea-table stea-table-readable">
          <thead>
            <tr>
              <th>Pattern</th>
              <th>Transactions</th>
            </tr>
          </thead>
          <tbody>
            {#each categorizationPatterns as row}
              <tr>
                <td>{row.pattern}</td>
                <td>{row.transaction_count}</td>
              </tr>
            {:else}
              <tr>
                <td colspan="2" class="stea-table-empty">No QR patterns</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}

  {#if activeTab === 'import'}
    <section class="stea-panel-grid">
      <div class="stea-form-grid">
        <label class="stea-field stea-span-all">
          <span class="stea-field-label">Bank statement PDF</span>
          <input
            class="stea-input"
            type="file"
            accept="application/pdf,.pdf"
            on:change={onFileChange}
          />
        </label>

        <div class="stea-toolbar stea-span-all">
          <button
            class="stea-btn-primary stea-btn-fit"
            type="button"
            disabled={!selectedFile || isUploading}
            on:click={uploadPreview}
          >
            <FileUp size={18} />
            {isUploading ? 'Parsing' : 'Parse preview'}
          </button>

          {#if selectedFile}
            <p class="stea-text">{selectedFile.name}</p>
          {/if}
        </div>
      </div>

      {#if error}
        <p class="stea-error">{error}</p>
      {/if}
    </section>
  {/if}

  {#if activeTab === 'import' && preview}
    <section class="stea-stack">
      <div class="stea-panel">
        <div class="stea-row-between">
          <div>
            <h2 class="stea-heading-sm">{preview.import.source_file_name}</h2>
            <p class="stea-text">
              {preview.import.bank} · {preview.import.account_currency}
              {#if preview.import.card_number_masked}
                · card {preview.import.card_number_masked}
              {/if}
            </p>
          </div>

          {#if preview.import.status === 'confirmed'}
            <span class="stea-badge stea-badge-buy">Confirmed</span>
          {:else}
            <button
              class="stea-btn-primary stea-btn-fit"
              type="button"
              disabled={isConfirming || newRows === 0}
              on:click={confirmImport}
            >
              <Check size={18} />
              {isConfirming ? 'Importing' : `Import ${newRows}`}
            </button>
          {/if}
        </div>
      </div>

      <dl class="stea-stat-grid">
        <div class="stea-stat-strip">
          <dt>Parsed</dt>
          <dd>{preview.rows.length}</dd>
        </div>
        <div class="stea-stat-strip">
          <dt>New</dt>
          <dd>{newRows}</dd>
        </div>
        <div class="stea-stat-strip">
          <dt>Duplicates</dt>
          <dd>{duplicateRows}</dd>
        </div>
      </dl>

      <div class="stea-table-wrap">
        <table class="stea-table stea-table-readable">
          <thead>
            <tr>
              <th>Date</th>
              <th>Description</th>
              <th>Kind</th>
              <th>Operation</th>
              <th>Fee</th>
              <th>Account</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {#each preview.rows as row}
              <tr>
                <td>{dateTime(row.occurred_at)}</td>
                <td>{row.description_raw}</td>
                <td>{kindLabel(row)}</td>
                <td>{money(row.operation_amount, row.operation_currency)}</td>
                <td>{money(row.fee_amount, row.fee_currency)}</td>
                <td>
                  {#if row.account_amount !== null && row.account_amount_currency}
                    {money(row.account_amount, row.account_amount_currency)}
                  {:else}
                    <span class="stea-muted">-</span>
                  {/if}
                </td>
                <td>
                  {#if row.is_duplicate}
                    <span class="stea-badge stea-badge-sell">Duplicate</span>
                  {:else}
                    <span class="stea-badge stea-badge-buy">New</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}

  {#if activeTab === 'rawtx'}
    <section class="stea-stack">
      <div class="stea-panel-grid">
        <div class="stea-row-between">
          <label class="stea-field stea-grow">
            <span class="stea-field-label">Search raw transactions</span>
            <input
              class="stea-input"
              type="search"
              bind:value={rawtxQuery}
              placeholder="description, card, kind, currency"
              on:input={scheduleRawtxSearch}
            />
          </label>
          <button
            class="stea-btn-secondary stea-btn-fit"
            type="button"
            disabled={isLoadingRawtx}
            on:click={loadRawtx}
          >
            Refresh
          </button>
        </div>

        <div class="stea-row-between">
          <p class="stea-text">
            {#if isLoadingRawtx}
              Loading
            {:else}
              {rawtxStart}-{rawtxEnd} of {rawtxTotal}
            {/if}
          </p>
          <div class="stea-row">
            <button
              class="stea-btn-secondary stea-btn-fit"
              type="button"
              disabled={rawtxOffset === 0 || isLoadingRawtx}
              on:click={previousRawtxPage}
            >
              Previous
            </button>
            <button
              class="stea-btn-secondary stea-btn-fit"
              type="button"
              disabled={rawtxOffset + rawtxLimit >= rawtxTotal || isLoadingRawtx}
              on:click={nextRawtxPage}
            >
              Next
            </button>
          </div>
        </div>

        {#if rawtxError}
          <p class="stea-error">{rawtxError}</p>
        {/if}
      </div>

      <div class="stea-table-wrap">
        <table class="stea-table stea-table-readable">
          <thead>
            <tr>
              <th>Date</th>
              <th>Description</th>
              <th>Kind</th>
              <th>Operation</th>
              <th>Fee</th>
              <th>Account</th>
              <th>Card</th>
              <th>Source</th>
            </tr>
          </thead>
          <tbody>
            {#each rawtxRows as row}
              <tr>
                <td>{dateTime(row.occurred_at)}</td>
                <td>{row.description_raw}</td>
                <td>{kindLabel(row)}</td>
                <td>{money(row.operation_amount, row.operation_currency)}</td>
                <td>{money(row.fee_amount, row.fee_currency)}</td>
                <td>
                  {#if row.account_amount !== null && row.account_amount_currency}
                    {money(row.account_amount, row.account_amount_currency)}
                  {:else}
                    <span class="stea-muted">-</span>
                  {/if}
                </td>
                <td>{row.card_number_masked ?? row.account_number_masked ?? '-'}</td>
                <td>{row.source_file_name}</td>
              </tr>
            {:else}
              <tr>
                <td colspan="8" class="stea-table-empty">No raw transactions</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}
</section>
