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

async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = await accessToken();
  const response = await fetch(`${apiBaseUrl}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...init.headers,
      Authorization: `Bearer ${token}`
    }
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed with ${response.status}`);
  }

  return response.json() as Promise<T>;
}
