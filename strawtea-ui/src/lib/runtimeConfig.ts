type RuntimeConfig = {
  VITE_API_BASE_URL?: string;
  VITE_SUPABASE_ANON_KEY?: string;
  VITE_SUPABASE_URL?: string;
};

declare global {
  interface Window {
    __STRAWTEA_CONFIG__?: RuntimeConfig;
  }
}

export function runtimeEnv(name: keyof RuntimeConfig): string {
  const runtimeValue = window.__STRAWTEA_CONFIG__?.[name]?.trim();
  if (runtimeValue) {
    return runtimeValue;
  }

  const buildEnv = import.meta.env as unknown as Record<string, string | undefined>;
  return buildEnv[name]?.trim() ?? '';
}
