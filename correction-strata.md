# AI-Linked Correction Screener

## Feature Purpose

Build a feature that helps the user discover stock tickers worth reviewing manually.

The feature should find companies that are meaningfully connected to the AI growth cycle, are still in a broad upward trend, but are currently experiencing a local downward correction.

The goal is not to recommend trades automatically.
The goal is to prepare a focused list of candidates for manual analysis.

---

## Core User Problem

The user wants to avoid manually scanning hundreds of tickers.

Instead, the app should automatically surface stocks that match this idea:

```text
AI-linked company
+ long-term upward trend
+ short-term correction
+ possible rebound setup
```

The user will then inspect the candidates and decide whether any of them are worth buying.

---

## Strategy Concept

The feature supports this investment/trading idea:

1. Find companies connected to AI, especially beyond the obvious first-tier names.
2. Prefer companies that are already in a broad upward trend.
3. Wait for a local pullback instead of buying during hype.
4. Highlight candidates that may be close to a rebound.
5. Let the user manually review the final list.

The feature should exclude news analysis.

---

## AI Connection Levels

Each company should be classified by how strongly it is connected to AI.

### Tier 1: Direct AI Leaders

Companies directly involved in core AI platforms, chips, cloud, or accelerated computing.

Examples:

* AI chip companies
* major cloud platforms
* foundational AI infrastructure companies
* companies with direct AI revenue exposure

### Tier 2: AI Infrastructure Suppliers

Companies that support the AI buildout but are not necessarily the main AI platforms.

Examples:

* semiconductor equipment
* memory
* networking
* data center hardware
* EDA software
* optical interconnect
* cloud infrastructure suppliers

### Tier 3: Indirect AI Beneficiaries

Companies that benefit from the physical and industrial expansion caused by AI demand.

Examples:

* data center power
* cooling systems
* electrical equipment
* industrial automation
* testing equipment
* specialty materials
* electronics supply chain
* infrastructure suppliers

---

## AI Relevance Score

Each ticker should receive an AI relevance score from `0` to `100`.

The score should reflect:

* how directly the company is connected to AI
* whether the sector and industry are relevant
* whether the company description mentions relevant AI infrastructure themes
* whether the company has reasonable business quality
* whether the stock has enough market liquidity
* whether the user has manually adjusted the score

The score should not be based only on whether the company mentions “AI”.

A company should not rank highly just because it uses AI-related marketing language.

---

## Trend Detection

The feature should only focus on companies that still appear to be in a broad upward trend.

A ticker should be considered healthy only if:

* the long-term trend is positive
* the stock has not collapsed too far from recent highs
* the company has enough trading volume
* the price is not too low
* the stock is not structurally weak compared with the broader market

The goal is to avoid companies that are falling because their long-term trend has already broken.

---

## Correction Detection

The feature should detect local downward corrections.

A good correction candidate is a stock that:

* is still in a broad upward trend
* has pulled back meaningfully from a recent high
* has not fallen too deeply
* is not showing extreme panic-like selling
* may be approaching a reasonable review zone

The correction should feel like a pullback inside a larger trend, not a complete breakdown.

---

## Candidate Statuses

Each ticker should have one of the following statuses.

### Ignore

The ticker does not currently match the strategy.

### Watch

The ticker is AI-relevant, still in a broad upward trend, and currently in a local correction.

This means it is worth watching, but not necessarily ready.

### Entry Candidate

The ticker was already in a correction and now shows signs of possible recovery.

This means it deserves closer manual review.

### Rejected

The ticker looked potentially relevant but failed an important filter.

Examples:

* trend is broken
* correction is too deep
* trading volume is too low
* price is too low
* technical condition is unhealthy

---

## Candidate Ranking

The feature should rank candidates so the most interesting ones appear first.

Priority should be:

1. Entry candidates
2. Watch candidates
3. Higher AI relevance score
4. Healthier long-term trend
5. Reasonable correction depth
6. Better liquidity
7. Stronger business quality

Rejected and ignored tickers should be shown only when the user wants to inspect them.

---

## Main User Flow

1. User opens the AI Correction Screener page.
2. User runs the screener.
3. App prepares a ranked list of candidates.
4. User sees which tickers are in `Watch`, `Entry Candidate`, `Rejected`, or `Ignore`.
5. User filters the list by status, AI tier, and AI score.
6. User opens a ticker detail view.
7. User reviews why the ticker matched or failed.
8. User can manually override AI tier or AI score.
9. User decides manually whether to add the ticker to a personal watchlist or trading journal.

---

## Screener Page

The page should show:

* latest screener run date
* button to run the screener
* ranked candidate table
* filters
* search by ticker
* ticker detail view
* manual override controls

---

## Table Columns

The main table should include:

* ticker
* company name
* AI tier
* AI score
* status
* current price
* correction depth
* distance from long-term trend level
* momentum condition
* volume condition
* rejection reason, if any

The table should be dense and practical, not overly visual.

---

## Ticker Detail View

The detail view should explain:

* what the company does
* why it is considered AI-linked
* assigned AI tier
* AI score breakdown
* current trend condition
* current correction condition
* why it matched the screener
* why it was rejected, if rejected
* manual notes
* manual AI tier and score override

The user should be able to understand the result without guessing why the ticker appeared.

---

## Manual Overrides

The user should be able to manually override:

* AI tier
* AI score
* notes about the company

This is important because AI relevance cannot be perfectly automated.

For example, the user may know that a company has stronger or weaker AI exposure than the automatic score suggests.

---

## What This Feature Should Not Do

Do not implement:

* news analysis
* automatic investment advice
* automatic buy or sell recommendations
* broker integration
* trade execution
* portfolio rebalancing
* real-time trading signals
* intraday scalping
* position management

The feature only prepares candidates for manual review.

---

## Success Criteria

The feature is complete when:

* user can run the AI-linked correction screener
* app returns a ranked list of candidate tickers
* each ticker has an AI tier and AI score
* each ticker has a clear status
* user can filter and inspect results
* user can understand why a ticker matched
* user can manually override AI tier and AI score
* news is not used
* no trading action is performed automatically

---

## Product Summary

This feature is a manual research assistant for AI-linked stock ideas.

It helps the user find stocks that may be interesting because they are:

```text
connected to AI
still in a larger uptrend
currently in a local pullback
possibly preparing for a rebound
```

The feature should reduce manual screening work while keeping the final investment decision fully under the user’s control.

