import { accessToken } from './auth';
import { runtimeEnv } from './runtimeConfig';

const apiBaseUrl = runtimeEnv('VITE_API_BASE_URL');

export type TickerSearchResult = {
  symbol: string;
  name: string;
  exchange: string | null;
  asset_type: string | null;
};

export type PricePoint = {
  date: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number | null;
};

export type PriceHistory = {
  ticker: string;
  range: '1mo';
  prices: PricePoint[];
};

export type CompanyAddress = {
  street1: string | null;
  street2: string | null;
  city: string | null;
  state_or_country: string | null;
  zip_code: string | null;
};

export type CompanyFiling = {
  form: string;
  filing_date: string | null;
  report_date: string | null;
  accession_number: string | null;
  primary_document: string | null;
  description: string | null;
  url: string | null;
};

export type CompanyFinancialMetric = {
  key: string;
  label: string;
  value: number;
  unit: string;
  fiscal_year: number | null;
  fiscal_period: string | null;
  form: string | null;
  filed: string | null;
  end: string | null;
  concept: string;
};

export type CompanyProfile = {
  ticker: string;
  cik: string;
  name: string;
  entity_type: string | null;
  sic: string | null;
  sic_description: string | null;
  exchanges: string[];
  tickers: string[];
  fiscal_year_end: string | null;
  state_of_incorporation: string | null;
  phone: string | null;
  business_address: CompanyAddress | null;
  mailing_address: CompanyAddress | null;
  sec_url: string;
  recent_filings: CompanyFiling[];
  financials: CompanyFinancialMetric[];
};

export type AiScreenerStatus = 'Ignore' | 'Watch' | 'Entry Candidate' | 'Rejected';
export type AiScreenerRunStatus =
  | 'queued'
  | 'running'
  | 'waiting_rate_limit'
  | 'stopped'
  | 'completed'
  | 'failed'
  | 'aborted';

export type AiScreenerResult = {
  id: string;
  run_id: string;
  run_completed_at: string | null;
  ticker: string;
  company_name: string;
  ai_tier: string | null;
  ai_score: number;
  status: AiScreenerStatus;
  current_price: number | null;
  correction_depth: number | null;
  trend_distance: number | null;
  momentum_condition: string;
  volume_condition: string;
  rejection_reason: string | null;
  rationale: {
    company_summary?: string;
    ai?: {
      deterministic_score?: number;
      theme_matches?: string[];
      provider?: string;
      enrichment_available?: boolean;
      reasons?: string[];
      warnings?: string[];
      confidence?: number;
    };
    technical?: string;
    technical_metrics?: {
      recovery_from_low?: number;
      days_since_low?: number;
    } | null;
    manual_override_applied?: boolean;
  };
  rank: number;
  manual_ai_tier: string | null;
  manual_ai_score: number | null;
  manual_status: AiScreenerStatus | null;
  manual_notes: string;
  processed_at: string;
};

export type BackgroundJobEvent = {
  id: string;
  event_type: string;
  message: string;
  payload: Record<string, unknown>;
  created_at: string;
};

export type AiScreenerRun = {
  id: string;
  job_id: string | null;
  status: AiScreenerRunStatus;
  run_after: string | null;
  status_reason: string | null;
  universe_count: number;
  processed_count: number;
  result_count: number;
  error: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
  results: AiScreenerResult[];
  events: BackgroundJobEvent[];
  latest_event: BackgroundJobEvent | null;
  twelve_budget_used: number;
  twelve_budget_limit: number;
};

export type UpdateAiScreenerOverride = {
  manual_ai_tier: string | null;
  manual_ai_score: number | null;
  manual_status: AiScreenerStatus | null;
  notes: string;
};

export type InvestlogEntry = {
  id: string;
  ticker: string;
  occurred_at: string;
  op: 'buy' | 'sell';
  broker: 'minvest';
  currency: 'USD';
  price: number;
  quantity: number;
  fees: number;
  notes: string;
  created_at: string;
  updated_at: string;
};

export type InvestlogAsset = {
  ticker: string;
  days_since_buy_midpoint: number;
  quantity: number;
  avg_buy_price: number;
  cost: number;
  current_price: number;
  price_change: number;
  current_value: number;
  amount_change: number;
  percent_change: number;
  price_fetched_at: string;
};

export type InvestlogAssetsSummary = {
  total_buys: number;
  total_sells: number;
  total_commissions: number;
  realized_profit: number;
  unrealized_profit: number;
  net_profit: number;
};

export type InvestlogAssets = {
  summary: InvestlogAssetsSummary;
  assets: InvestlogAsset[];
};

export type InvestlogWatchlistItem = {
  id: string;
  ticker: string;
  company_name: string | null;
  description: string | null;
  market_cap: number | null;
  shares_outstanding: number | null;
  revenue: number | null;
  total_debt: number | null;
  cash: number | null;
  free_cash_flow: number | null;
  current_price: number | null;
  currency: string | null;
  notes: InvestlogTickerNote[];
  is_active: boolean;
  removed_at: string | null;
  meta_fetched_at: string | null;
  price_fetched_at: string | null;
  created_at: string;
  updated_at: string;
};

export type InvestlogTickerNote = {
  id: string;
  ticker: string;
  note: string;
  created_at: string;
};

export type InvestlogPerformancePoint = {
  date: string;
  close: number;
  index: number;
};

export type InvestlogPerformanceSeries = {
  ticker: string;
  points: InvestlogPerformancePoint[];
};

export type InvestlogPerformanceEvent = {
  ticker: string;
  date: string;
  op: 'buy' | 'sell';
  price: number;
  quantity: number;
  notes: string;
};

export type InvestlogReportEvent = {
  ticker: string;
  date: string;
  form: '10-K' | '10-Q';
  filing_date: string | null;
};

export type InvestlogPerformance = {
  tickers: string[];
  range: InvestlogPerformanceRange;
  series: InvestlogPerformanceSeries[];
  events: InvestlogPerformanceEvent[];
  report_events: InvestlogReportEvent[];
  ticker_notes: InvestlogTickerNote[];
};

export type InvestlogPerformanceRange = '1m' | '3m' | '6m' | '1y' | '3y';

export type SettingResponse<T> = {
  value: T | null;
};

export type PushKeyResponse = {
  enabled: boolean;
  public_key: string | null;
};

export type PushSubscriptionPayload = {
  endpoint: string;
  keys: {
    p256dh: string;
    auth: string;
  };
};

export type PushSubscriptionResponse = {
  id: string;
};

export type TestNotificationResponse = {
  sent_count: number;
  failed_count: number;
};

export type CreateInvestlogEntry = {
  ticker: string;
  occurred_at: string;
  op: 'buy' | 'sell';
  broker: 'minvest';
  currency: 'USD';
  price: number;
  quantity: number;
  fees: number;
  notes: string;
};

export type UpdateInvestlogEntry = CreateInvestlogEntry;

export type AddWatchlistItem = {
  ticker: string;
  note: string;
};

export type WatchlistRemoval = {
  note: string;
};

export type CreateInvestlogTickerNote = {
  ticker: string;
  note: string;
};

export type RawtxImport = {
  id: string;
  source_file_name: string;
  source_file_sha256: string;
  parser_name: string;
  parser_version: number;
  bank: string;
  account_number_masked: string | null;
  card_number_masked: string | null;
  account_currency: string;
  statement_period_start: string | null;
  statement_period_end: string | null;
  status: 'previewed' | 'confirmed' | 'failed';
  rows_seen: number;
  rows_inserted: number;
  rows_duplicate: number;
  error: string | null;
  created_at: string;
  confirmed_at: string | null;
};

export type RawtxPreviewRow = {
  id: string;
  row_index: number;
  occurred_at: string;
  posted_date: string | null;
  description_raw: string;
  operation_amount: number;
  operation_currency: string;
  fee_amount: number;
  fee_currency: string;
  account_amount: number | null;
  account_amount_currency: string | null;
  direction: 'debit' | 'credit' | 'neutral';
  raw_kind: string | null;
  is_duplicate: boolean;
};

export type RawtxImportPreview = {
  import: RawtxImport;
  rows: RawtxPreviewRow[];
};

export type RawtxRow = {
  id: string;
  source_file_name: string;
  bank: string;
  account_number_masked: string | null;
  card_number_masked: string | null;
  account_currency: string;
  occurred_at: string;
  posted_date: string | null;
  description_raw: string;
  operation_amount: number;
  operation_currency: string;
  fee_amount: number;
  fee_currency: string;
  account_amount: number | null;
  account_amount_currency: string | null;
  direction: 'debit' | 'credit' | 'neutral';
  raw_kind: string | null;
  parser_name: string;
  parser_version: number;
  created_at: string;
};

export type RawtxList = {
  rows: RawtxRow[];
  total: number;
  limit: number;
  offset: number;
};

export type RawtxMonthlySpend = {
  month: string;
  currency: string;
  amount: number;
  transaction_count: number;
};

export type RawtxCategorizationPattern = {
  pattern: string;
  transaction_count: number;
};

export async function searchTickers(query: string): Promise<TickerSearchResult[]> {
  return apiFetch(`/api/stocks/search?q=${encodeURIComponent(query)}`);
}

export async function fetchPriceHistory(ticker: string): Promise<PriceHistory> {
  return apiFetch(`/api/stocks/${encodeURIComponent(ticker)}/prices?range=1mo`);
}

export async function fetchCompanyProfile(ticker: string): Promise<CompanyProfile> {
  return apiFetch(`/api/stocks/${encodeURIComponent(ticker)}/profile`);
}

export async function startAiCorrectionScreenerRun(): Promise<AiScreenerRun> {
  return apiFetch('/api/ai-correction-screener/runs', {
    method: 'POST'
  });
}

export async function fetchLatestAiCorrectionScreenerRun(): Promise<AiScreenerRun | null> {
  return apiFetch('/api/ai-correction-screener/runs/latest');
}

export async function updateAiCorrectionOverride(
  ticker: string,
  payload: UpdateAiScreenerOverride
): Promise<AiScreenerRun> {
  return apiFetch(`/api/ai-correction-screener/overrides/${encodeURIComponent(ticker)}`, {
    method: 'PUT',
    body: JSON.stringify(payload)
  });
}

export async function stopJob(jobId: string): Promise<unknown> {
  return apiFetch(`/api/jobs/${encodeURIComponent(jobId)}/stop`, {
    method: 'POST'
  });
}

export async function resumeJob(jobId: string): Promise<unknown> {
  return apiFetch(`/api/jobs/${encodeURIComponent(jobId)}/resume`, {
    method: 'POST'
  });
}

export async function abortJob(jobId: string): Promise<unknown> {
  return apiFetch(`/api/jobs/${encodeURIComponent(jobId)}/abort`, {
    method: 'POST'
  });
}

export async function listInvestlogEntries(): Promise<InvestlogEntry[]> {
  return apiFetch('/api/investlog');
}

export async function listInvestlogAssets(): Promise<InvestlogAssets> {
  return apiFetch('/api/investlog/assets');
}

export async function listInvestlogWatchlist(): Promise<InvestlogWatchlistItem[]> {
  return apiFetch('/api/investlog/watchlist');
}

export async function addInvestlogWatchlistItem(
  payload: AddWatchlistItem
): Promise<InvestlogWatchlistItem> {
  return apiFetch('/api/investlog/watchlist', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}

export async function removeInvestlogWatchlistItem(
  ticker: string,
  payload: WatchlistRemoval
): Promise<InvestlogWatchlistItem> {
  return apiFetch(`/api/investlog/watchlist/${encodeURIComponent(ticker)}/remove`, {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}

export async function createInvestlogTickerNote(
  payload: CreateInvestlogTickerNote
): Promise<InvestlogTickerNote> {
  return apiFetch('/api/investlog/notes', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}

export async function fetchInvestlogPerformance(
  tickers: string[],
  range: InvestlogPerformanceRange = '1y'
): Promise<InvestlogPerformance> {
  const params = new URLSearchParams({
    ticker: tickers.join(','),
    range
  });

  return apiFetch(`/api/investlog/performance?${params}`);
}

export async function createInvestlogEntry(
  entry: CreateInvestlogEntry
): Promise<InvestlogEntry> {
  return apiFetch('/api/investlog', {
    method: 'POST',
    body: JSON.stringify(entry)
  });
}

export async function updateInvestlogEntry(
  id: string,
  entry: UpdateInvestlogEntry
): Promise<InvestlogEntry> {
  return apiFetch(`/api/investlog/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(entry)
  });
}

export async function getSettingValue<T>(
  section: string,
  key: string
): Promise<T | null> {
  const response = await apiFetch<SettingResponse<T>>(
    `/api/settings/${encodeURIComponent(section)}/${encodeURIComponent(key)}`
  );

  return response.value;
}

export async function putSettingValue<T>(
  section: string,
  key: string,
  value: T
): Promise<T | null> {
  const response = await apiFetch<SettingResponse<T>>(
    `/api/settings/${encodeURIComponent(section)}/${encodeURIComponent(key)}`,
    {
      method: 'PUT',
      body: JSON.stringify({ value })
    }
  );

  return response.value;
}

export async function fetchPushPublicKey(): Promise<PushKeyResponse> {
  return apiFetch('/api/notifications/push-key');
}

export async function savePushSubscription(
  payload: PushSubscriptionPayload
): Promise<PushSubscriptionResponse> {
  return apiFetch('/api/notifications/push-subscriptions', {
    method: 'POST',
    body: JSON.stringify(payload)
  });
}

export async function sendTestNotification(): Promise<TestNotificationResponse> {
  return apiFetch('/api/notifications/test', {
    method: 'POST'
  });
}

export async function previewRawtxImport(file: File): Promise<RawtxImportPreview> {
  const formData = new FormData();
  formData.set('file', file);

  return apiFetch('/api/spends/imports/preview', {
    method: 'POST',
    body: formData
  });
}

export async function confirmRawtxImport(importId: string): Promise<RawtxImportPreview> {
  return apiFetch(`/api/spends/imports/${encodeURIComponent(importId)}/confirm`, {
    method: 'POST'
  });
}

export async function listRawtx(options: {
  q?: string;
  limit?: number;
  offset?: number;
} = {}): Promise<RawtxList> {
  const params = new URLSearchParams();
  if (options.q?.trim()) params.set('q', options.q.trim());
  if (options.limit) params.set('limit', String(options.limit));
  if (options.offset) params.set('offset', String(options.offset));

  const suffix = params.toString();
  return apiFetch(`/api/spends/rawtx${suffix ? `?${suffix}` : ''}`);
}

export async function listMonthlySpends(): Promise<RawtxMonthlySpend[]> {
  return apiFetch('/api/spends/monthly');
}

export async function listCategorizationPatterns(): Promise<RawtxCategorizationPattern[]> {
  return apiFetch('/api/spends/categorization/patterns');
}

async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = await accessToken();
  const headers = new Headers(init.headers);
  if (!(init.body instanceof FormData) && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  headers.set('Authorization', `Bearer ${token}`);

  const response = await fetch(`${apiBaseUrl}${path}`, {
    ...init,
    headers
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed with ${response.status}`);
  }

  return response.json() as Promise<T>;
}
