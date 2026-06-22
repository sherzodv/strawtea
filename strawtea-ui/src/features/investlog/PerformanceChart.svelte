<script lang="ts">
  import type { InvestlogPerformance } from '../../lib/api';
  import type { AnalysisIntervalSetting } from '../../lib/settings';
  import { chartColorForAssetIndex, chartColors } from './chartColors';

  export let performance: InvestlogPerformance;
  export let assetTickers: string[] = [];
  export let colorTickers: string[] = [];
  export let interval: AnalysisIntervalSetting | null = null;
  export let onIntervalChange: (interval: AnalysisIntervalSetting | null) => void = () => {};

  const width = 100;
  const height = 100;
  type ChartTargetPoint = {
    ticker: string;
    date: string;
    close: number;
    index: number;
    x: number;
    y: number;
  };

  let anchorPoint: ChartTargetPoint | null = null;
  let comparePoint: ChartTargetPoint | null = null;
  let activeTarget: 'anchor' | 'compare' | null = null;
  let nextTarget: 'anchor' | 'compare' = 'anchor';
  let appliedIntervalKey = '';

  $: values = performance.series.flatMap((series) => series.points.map((point) => point.index));
  $: min = Math.min(...values, 95);
  $: max = Math.max(...values, 105);
  $: spread = Math.max(max - min, 1);
  $: targetSeries =
    performance.series.find((series) => isAssetTicker(series.ticker) && series.points.length > 0) ??
    performance.series.find((series) => series.points.length > 0);
  $: targetSeriesPoints =
    targetSeries?.points.map((point) => ({
      ticker: targetSeries.ticker,
      date: point.date,
      close: point.close,
      index: point.index,
      x: xForPoint(targetSeries.points, point.date),
      y: yForValue(point.index)
    })) ?? [];
  $: applyIntervalSetting(interval, targetSeriesPoints);
  $: intervalStartX =
    anchorPoint && comparePoint ? Math.min(anchorPoint.x, comparePoint.x) : 0;
  $: intervalWidth =
    anchorPoint && comparePoint ? Math.abs(anchorPoint.x - comparePoint.x) : 0;
  $: intervalChange =
    anchorPoint && comparePoint ? comparePoint.close - anchorPoint.close : 0;
  $: intervalChangePercent =
    anchorPoint && comparePoint && anchorPoint.close > 0
      ? ((comparePoint.close - anchorPoint.close) / anchorPoint.close) * 100
      : 0;
  $: intervalDays =
    anchorPoint && comparePoint
      ? Math.abs(
          Math.round(
            (new Date(comparePoint.date).getTime() - new Date(anchorPoint.date).getTime()) /
              86_400_000
          )
        )
      : 0;
  $: anchorPriceLabelPosition =
    anchorPoint && comparePoint && anchorPoint.y <= comparePoint.y ? 'above' : 'below';
  $: comparePriceLabelPosition =
    anchorPoint && comparePoint && comparePoint.y <= anchorPoint.y ? 'above' : 'below';
  $: eventPoints = performance.events
    .map((event) => {
      const series = performance.series.find((item) => item.ticker === event.ticker);
      if (!series || series.points.length === 0) {
        return null;
      }

      const index = series.points.findIndex((point) => point.date >= event.date);
      const point = series.points[index < 0 ? series.points.length - 1 : index];

      if (!point) {
        return null;
      }

      return {
        ...event,
        x: xForPoint(series.points, point.date),
        y: yForValue(point.index)
      };
    })
    .filter(Boolean) as Array<
    {
      ticker: string;
      date: string;
      op: 'buy' | 'sell';
      price: number;
      quantity: number;
      notes: string;
      x: number;
      y: number;
    }
  >;
  $: reportEventPoints = performance.report_events
    .map((event) => {
      const series = performance.series.find((item) => item.ticker === event.ticker);
      if (!series || series.points.length === 0) {
        return null;
      }

      const index = series.points.findIndex((point) => point.date >= event.date);
      const point = series.points[index < 0 ? series.points.length - 1 : index];

      if (!point) {
        return null;
      }

      return {
        ...event,
        x: xForPoint(series.points, point.date),
        color: colorForSeries(event.ticker, performance.series.indexOf(series))
      };
    })
    .filter(Boolean) as Array<
    {
      ticker: string;
      date: string;
      form: '10-K' | '10-Q';
      filing_date: string | null;
      x: number;
      color: string;
    }
  >;

  function linePoints(points: Array<{ date: string; index: number }>) {
    return points
      .map((point) => `${xForPoint(points, point.date)},${yForValue(point.index)}`)
      .join(' ');
  }

  function xForPoint(points: Array<{ date: string }>, date: string) {
    const index = points.findIndex((point) => point.date === date);
    return points.length === 1 ? 0 : (index / (points.length - 1)) * width;
  }

  function yForValue(value: number) {
    return height - ((value - min) / spread) * 90 - 5;
  }

  function formatMoney(value: number) {
    const amount = (value / 100).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });

    return `${amount} $`;
  }

  function formatPercent(value: number) {
    return `${value.toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    })}%`;
  }

  function colorForSeries(ticker: string, index: number) {
    const colorIndex = colorTickers.indexOf(ticker);
    return colorIndex >= 0 ? chartColorForAssetIndex(colorIndex) : chartColors[index % chartColors.length];
  }

  function isAssetTicker(ticker: string) {
    return assetTickers.includes(ticker);
  }

  function reportFormLabel(form: '10-K' | '10-Q') {
    return form === '10-K' ? 'Y' : 'Q';
  }

  function reportFormDescription(form: '10-K' | '10-Q') {
    return form === '10-K' ? 'yearly' : 'quarter';
  }

  function pointForEvent(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const x = Math.min(Math.max(((event.clientX - rect.left) / rect.width) * 100, 0), 100);
    return targetSeriesPoints.reduce<ChartTargetPoint | null>((nearest, point) => {
      if (!nearest) {
        return point;
      }

      const nearestDistance = Math.abs(nearest.x - x);
      const pointDistance = Math.abs(point.x - x);

      return pointDistance < nearestDistance ? point : nearest;
    }, null);
  }

  function pointForDate(date: string) {
    const targetTime = Date.parse(date);

    if (!Number.isFinite(targetTime)) {
      return null;
    }

    return targetSeriesPoints.reduce<ChartTargetPoint | null>((nearest, point) => {
      if (!nearest) {
        return point;
      }

      const nearestDistance = Math.abs(Date.parse(nearest.date) - targetTime);
      const pointDistance = Math.abs(Date.parse(point.date) - targetTime);

      return pointDistance < nearestDistance ? point : nearest;
    }, null);
  }

  function applyIntervalSetting(
    value: AnalysisIntervalSetting | null,
    points: ChartTargetPoint[]
  ) {
    const pointsKey = `${points.length}:${points[0]?.date ?? ''}:${points.at(-1)?.date ?? ''}`;
    const nextKey = `${value?.anchorDate ?? ''}:${value?.compareDate ?? ''}:${value?.nextTarget ?? ''}:${pointsKey}`;

    if (nextKey === appliedIntervalKey) {
      return;
    }

    appliedIntervalKey = nextKey;
    anchorPoint = value?.anchorDate ? pointForDate(value.anchorDate) : null;
    comparePoint = value?.compareDate ? pointForDate(value.compareDate) : null;
    nextTarget = value?.nextTarget ?? 'anchor';
  }

  function setActiveTargetPoint(event: PointerEvent) {
    const nearestPoint = pointForEvent(event);
    if (!nearestPoint || !activeTarget) {
      return;
    }

    if (activeTarget === 'anchor') {
      anchorPoint = nearestPoint;
      return;
    }

    comparePoint = nearestPoint;
  }

  function startTargetGesture(event: PointerEvent) {
    event.preventDefault();
    activeTarget = nextTarget;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    setActiveTargetPoint(event);
  }

  function dragTargetGesture(event: PointerEvent) {
    setActiveTargetPoint(event);
  }

  function finishTargetGesture(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;

    if (target.hasPointerCapture(event.pointerId)) {
      target.releasePointerCapture(event.pointerId);
    }

    if (activeTarget) {
      nextTarget = activeTarget === 'anchor' ? 'compare' : 'anchor';
    }

    activeTarget = null;
    onIntervalChange(currentIntervalSetting());
  }

  function currentIntervalSetting(): AnalysisIntervalSetting | null {
    if (!anchorPoint || !comparePoint) {
      return null;
    }

    return {
      anchorDate: anchorPoint.date,
      compareDate: comparePoint.date,
      nextTarget
    };
  }
</script>

{#if performance.series.length === 0 || performance.series.every((series) => series.points.length === 0)}
  <p class="stea-muted">No performance data available.</p>
{:else}
  <div class="stea-panel">
    <div class="stea-chart-wrap">
      <svg class="stea-chart" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label="Ticker versus benchmark performance chart">
        {#each performance.series as series, index}
          <polyline
            points={linePoints(series.points)}
            fill="none"
            stroke={colorForSeries(series.ticker, index)}
            stroke-width={isAssetTicker(series.ticker) ? 3 : 2}
            vector-effect="non-scaling-stroke"
          />
        {/each}
      </svg>
      {#each eventPoints as event}
        <span
          class={event.op === 'buy' ? 'stea-chart-marker stea-chart-marker-buy' : 'stea-chart-marker stea-chart-marker-sell'}
          style={`left: ${event.x}%; top: ${event.y}%;`}
          aria-label={`${event.op} ${event.date}`}
          role="img"
        >
          <span class="stea-sr-only">{event.op.toUpperCase()} {event.date}: {formatMoney(event.price)} - {event.notes}</span>
        </span>
      {/each}
      {#each reportEventPoints as event}
        <span
          class="stea-chart-report-line"
          style={`left: ${event.x}%; --stea-report-color: ${event.color};`}
          aria-label={`${event.ticker} ${reportFormDescription(event.form)} report date ${event.date}`}
          role="img"
        >
          <span class="stea-chart-report-label">{reportFormLabel(event.form)}</span>
          <span class="stea-sr-only">
            {event.ticker} {reportFormDescription(event.form)} report date {event.date}
            {#if event.filing_date}
              filed {event.filing_date}
            {/if}
          </span>
        </span>
      {/each}
      <button
        class="stea-chart-target-layer"
        type="button"
        aria-label="Move chart target"
        on:pointerdown={startTargetGesture}
        on:pointermove={dragTargetGesture}
        on:pointerup={finishTargetGesture}
        on:pointercancel={finishTargetGesture}
      >
        {#if anchorPoint && comparePoint}
          <span class="stea-chart-target-interval" style={`left: ${intervalStartX}%; width: ${intervalWidth}%;`}></span>
          <span
            class={intervalChangePercent >= 0 ? 'stea-chart-target-change stea-gain' : 'stea-chart-target-change stea-loss'}
          >
            {formatPercent(intervalChangePercent)} · {formatMoney(intervalChange)} · {intervalDays}d
          </span>
        {/if}
        {#if anchorPoint}
          <span class="stea-chart-target-x stea-chart-target-x-anchor" style={`left: ${anchorPoint.x}%;`}></span>
          <span class="stea-chart-target-y stea-chart-target-y-anchor" style={`top: ${anchorPoint.y}%;`}></span>
          <span class="stea-chart-target-dot stea-chart-target-dot-anchor" style={`left: ${anchorPoint.x}%; top: ${anchorPoint.y}%;`}></span>
          <span class="stea-chart-target-label stea-chart-target-label-x" style={`left: ${anchorPoint.x}%;`}>{anchorPoint.date}</span>
          <span
            class={anchorPriceLabelPosition === 'above'
              ? 'stea-chart-target-label stea-chart-target-price-label stea-chart-target-price-label-above'
              : 'stea-chart-target-label stea-chart-target-price-label stea-chart-target-price-label-below'}
            style={`top: ${anchorPoint.y}%;`}
          >
            {anchorPoint.ticker} {formatMoney(anchorPoint.close)}
          </span>
        {/if}
        {#if comparePoint}
          <span class="stea-chart-target-x" style={`left: ${comparePoint.x}%;`}></span>
          <span class="stea-chart-target-y" style={`top: ${comparePoint.y}%;`}></span>
          <span class="stea-chart-target-dot" style={`left: ${comparePoint.x}%; top: ${comparePoint.y}%;`}></span>
          <span
            class={comparePriceLabelPosition === 'above'
              ? 'stea-chart-target-label stea-chart-target-price-label stea-chart-target-price-label-above'
              : 'stea-chart-target-label stea-chart-target-price-label stea-chart-target-price-label-below'}
            style={`top: ${comparePoint.y}%;`}
          >
            {comparePoint.ticker} {formatMoney(comparePoint.close)}
          </span>
          <span class="stea-chart-target-label stea-chart-target-label-x" style={`left: ${comparePoint.x}%;`}>{comparePoint.date}</span>
        {/if}
      </button>
    </div>

    <div class="stea-meta-row stea-meta-row-sm">
      <span>{performance.series[0]?.points[0]?.date}</span>
      <span>{performance.series[0]?.points.at(-1)?.date}</span>
    </div>
  </div>
{/if}
