import { accessToken } from './auth';

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:8080';

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

export async function searchTickers(query: string): Promise<TickerSearchResult[]> {
  return apiFetch(`/api/stocks/search?q=${encodeURIComponent(query)}`);
}

export async function fetchPriceHistory(ticker: string): Promise<PriceHistory> {
  return apiFetch(`/api/stocks/${encodeURIComponent(ticker)}/prices?range=1mo`);
}

async function apiFetch<T>(path: string): Promise<T> {
  const token = await accessToken();
  const response = await fetch(`${apiBaseUrl}${path}`, {
    headers: {
      Authorization: `Bearer ${token}`
    }
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed with ${response.status}`);
  }

  return response.json() as Promise<T>;
}
