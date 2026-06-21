import { getSettingValue, putSettingValue } from './api';
import type { InvestlogPerformanceRange } from './api';

export type InvestlogTab = 'assets' | 'analysis' | 'history';

export type AnalysisIntervalSetting = {
  anchorDate: string | null;
  compareDate: string | null;
  nextTarget: 'anchor' | 'compare';
};

export type InvestlogPageSettings = {
  activeTab: InvestlogTab;
};

export type InvestlogAnalysisSettings = {
  selectedTickers: string[];
  recentTickers: string[];
  range: InvestlogPerformanceRange;
  interval: AnalysisIntervalSetting | null;
  tickerHistory?: string[];
};

class SyncedSetting<T extends Record<string, unknown>> {
  private readonly localKey: string;

  constructor(
    private readonly section: string,
    private readonly key: string,
    private readonly defaults: T
  ) {
    this.localKey = `strawtea:settings:${section}:${key}`;
  }

  getCached(): T {
    const localValue = this.readLocal();
    return this.withDefaults(localValue);
  }

  async load(): Promise<T> {
    const cached = this.getCached();

    try {
      const remote = await getSettingValue<T>(this.section, this.key);

      if (remote !== null) {
        const value = this.withDefaults(remote);
        this.writeLocal(value);
        return value;
      }

      await this.save(cached);
    } catch (err) {
      console.warn(`Could not sync ${this.section}/${this.key} settings`, err);
    }

    return cached;
  }

  async save(value: T): Promise<void> {
    const next = this.withDefaults(value);
    this.writeLocal(next);

    try {
      await putSettingValue(this.section, this.key, next);
    } catch (err) {
      console.warn(`Could not save ${this.section}/${this.key} settings`, err);
    }
  }

  async patch(value: Partial<T>): Promise<T> {
    const next = this.withDefaults({
      ...this.getCached(),
      ...value
    } as T);
    await this.save(next);
    return next;
  }

  private readLocal(): T | null {
    try {
      const item = localStorage.getItem(this.localKey);
      return item ? (JSON.parse(item) as T) : null;
    } catch {
      return null;
    }
  }

  private writeLocal(value: T) {
    localStorage.setItem(this.localKey, JSON.stringify(value));
  }

  private withDefaults(value: T | null): T {
    return {
      ...this.defaults,
      ...(value ?? {})
    };
  }
}

class AppSettings {
  readonly investlog = {
    page: new SyncedSetting<InvestlogPageSettings>('investlog', 'page', {
      activeTab: 'assets'
    }),
    analysis: new SyncedSetting<InvestlogAnalysisSettings>('investlog', 'analysis', {
      selectedTickers: [],
      recentTickers: [],
      range: '6m',
      interval: null
    })
  };
}

export const appSettings = new AppSettings();
