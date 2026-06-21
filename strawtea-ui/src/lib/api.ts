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

export type InvestlogPerformance = {
  tickers: string[];
  range: InvestlogPerformanceRange;
  series: InvestlogPerformanceSeries[];
  events: InvestlogPerformanceEvent[];
};

export type InvestlogPerformanceRange = '1m' | '3m' | '6m' | '1y' | '3y';

export type SettingResponse<T> = {
  value: T | null;
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

export async function listInvestlogEntries(): Promise<InvestlogEntry[]> {
  return apiFetch('/api/investlog');
}

export async function listInvestlogAssets(): Promise<InvestlogAsset[]> {
  return apiFetch('/api/investlog/assets');
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
