use std::{collections::HashMap, sync::Arc};

use chrono::NaiveDate;
use reqwest::Url;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};

use crate::{
    error::AppError,
    integrations::throttle::ProviderThrottle,
    models::{CompanyAddress, CompanyFiling, CompanyFinancialMetric, CompanyProfile},
};

#[derive(Debug, Clone)]
pub struct EdgarTickerCompany {
    pub ticker: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct EdgarCompanyProfileSummary {
    pub name: String,
    pub sic_description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EdgarFinancialSnapshot {
    pub shares_outstanding: Option<i64>,
    pub revenue: Option<i64>,
    pub total_debt: Option<i64>,
    pub cash: Option<i64>,
    pub free_cash_flow: Option<i64>,
}

#[derive(Clone)]
pub struct EdgarClient {
    client: reqwest::Client,
    ticker_index: Arc<OnceCell<Vec<SecTickerCompany>>>,
    profile_summary_cache: Arc<RwLock<HashMap<String, EdgarCompanyProfileSummary>>>,
    throttle: ProviderThrottle,
}

impl EdgarClient {
    pub fn new(user_agent: String, throttle: ProviderThrottle) -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            ticker_index: Arc::new(OnceCell::new()),
            profile_summary_cache: Arc::new(RwLock::new(HashMap::new())),
            throttle,
        })
    }

    pub async fn company_profile(&self, ticker: &str) -> Result<CompanyProfile, AppError> {
        let ticker = ticker.trim().to_uppercase();
        if ticker.is_empty() {
            return Err(AppError::BadRequest("ticker is required".to_string()));
        }

        let company = self
            .company_for_ticker(&ticker)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("{ticker} was not found in SEC EDGAR")))?;
        let cik = format!("{:010}", company.cik_str);
        let submissions = self.submissions(&cik).await?;
        let recent_filings = submissions.recent_filings(&cik, 6);
        let financials = match self.company_financials(&cik).await {
            Ok(financials) => financials,
            Err(err @ AppError::RateLimited { .. }) => return Err(err),
            Err(_) => Vec::new(),
        };

        Ok(CompanyProfile {
            ticker,
            cik: cik.clone(),
            name: non_empty(submissions.name).unwrap_or(company.title),
            entity_type: non_empty(submissions.entity_type),
            sic: non_empty(submissions.sic),
            sic_description: non_empty(submissions.sic_description),
            exchanges: submissions.exchanges.unwrap_or_default(),
            tickers: submissions.tickers.unwrap_or_default(),
            fiscal_year_end: non_empty(submissions.fiscal_year_end),
            state_of_incorporation: non_empty(submissions.state_of_incorporation_description)
                .or_else(|| non_empty(submissions.state_of_incorporation)),
            phone: non_empty(submissions.phone),
            business_address: submissions.addresses.business.map(Into::into),
            mailing_address: submissions.addresses.mailing.map(Into::into),
            sec_url: format!("https://www.sec.gov/edgar/browse/?CIK={cik}&owner=exclude"),
            recent_filings,
            financials,
        })
    }

    pub async fn company_profile_summary(
        &self,
        company: &EdgarTickerCompany,
    ) -> Result<EdgarCompanyProfileSummary, AppError> {
        let ticker = company.ticker.trim().replace('.', "-").to_uppercase();
        if let Some(summary) = self
            .profile_summary_cache
            .read()
            .await
            .get(&ticker)
            .cloned()
        {
            return Ok(summary);
        }

        let Some(index_company) = self.company_for_ticker(&company.ticker).await? else {
            let summary = EdgarCompanyProfileSummary {
                name: company.name.clone(),
                sic_description: None,
            };
            self.profile_summary_cache
                .write()
                .await
                .insert(ticker, summary.clone());
            return Ok(summary);
        };

        let cik = format!("{:010}", index_company.cik_str);
        let submissions = self.submissions(&cik).await?;
        let summary = EdgarCompanyProfileSummary {
            name: non_empty(submissions.name).unwrap_or_else(|| company.name.clone()),
            sic_description: non_empty(submissions.sic_description),
        };
        self.profile_summary_cache
            .write()
            .await
            .insert(ticker, summary.clone());

        Ok(summary)
    }

    pub async fn has_profile_summary(&self, ticker: &str) -> bool {
        let ticker = ticker.trim().replace('.', "-").to_uppercase();
        self.profile_summary_cache
            .read()
            .await
            .contains_key(&ticker)
    }

    pub async fn financial_snapshot(
        &self,
        ticker: &str,
    ) -> Result<EdgarFinancialSnapshot, AppError> {
        let ticker = ticker.trim().to_uppercase();
        if ticker.is_empty() {
            return Ok(EdgarFinancialSnapshot::default());
        }

        let Some(company) = self.company_for_ticker(&ticker).await? else {
            return Ok(EdgarFinancialSnapshot::default());
        };
        let cik = format!("{:010}", company.cik_str);
        let facts = self.company_financials_raw(&cik).await?;
        let metrics = financial_metrics(&facts);

        Ok(EdgarFinancialSnapshot {
            shares_outstanding: metric_i64(&metrics, "shares_outstanding"),
            revenue: metric_usd_cents(&metrics, "revenue"),
            total_debt: metric_usd_cents(&metrics, "total_debt"),
            cash: metric_usd_cents(&metrics, "cash"),
            free_cash_flow: metric_usd_cents(&metrics, "free_cash_flow"),
        })
    }

    pub async fn report_events(
        &self,
        ticker: &str,
        start_date: NaiveDate,
    ) -> Result<Vec<EdgarReportEvent>, AppError> {
        let ticker = ticker.trim().to_uppercase();
        if ticker.is_empty() {
            return Ok(Vec::new());
        }

        let Some(company) = self.company_for_ticker(&ticker).await? else {
            return Ok(Vec::new());
        };

        let cik = format!("{:010}", company.cik_str);
        let submissions = self.submissions(&cik).await?;

        Ok(submissions.report_events(&ticker, start_date))
    }

    pub async fn ticker_companies(&self) -> Result<Vec<EdgarTickerCompany>, AppError> {
        let companies = self
            .ticker_index
            .get_or_try_init(|| async { self.fetch_ticker_index().await })
            .await?;

        Ok(companies
            .iter()
            .map(|company| EdgarTickerCompany {
                ticker: company.ticker.clone(),
                name: company.title.clone(),
            })
            .collect())
    }

    async fn company_financials(&self, cik: &str) -> Result<Vec<CompanyFinancialMetric>, AppError> {
        let facts = self.company_financials_raw(cik).await?;
        Ok(financial_metrics(&facts))
    }

    async fn company_financials_raw(&self, cik: &str) -> Result<SecCompanyFacts, AppError> {
        let cik_path = format!("CIK{cik}.json");
        let mut url = Url::parse("https://data.sec.gov/api/xbrl/companyfacts/")
            .map_err(|err| AppError::Edgar(err.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| AppError::Edgar("invalid SEC companyfacts base URL".to_string()))?
            .push(&cik_path);

        self.throttle
            .reserve("sec_edgar", "companyfacts", 1)
            .await?;
        let facts = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<SecCompanyFacts>()
            .await?;

        Ok(facts)
    }

    async fn submissions(&self, cik: &str) -> Result<SecSubmissions, AppError> {
        let cik_path = format!("CIK{cik}.json");
        let mut url = Url::parse("https://data.sec.gov/submissions/")
            .map_err(|err| AppError::Edgar(err.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| AppError::Edgar("invalid SEC submissions base URL".to_string()))?
            .push(&cik_path);

        self.throttle.reserve("sec_edgar", "submissions", 1).await?;

        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<SecSubmissions>()
            .await
            .map_err(Into::into)
    }

    async fn company_for_ticker(&self, ticker: &str) -> Result<Option<SecTickerCompany>, AppError> {
        let companies = self
            .ticker_index
            .get_or_try_init(|| async { self.fetch_ticker_index().await })
            .await?;
        let edgar_ticker = ticker.replace('.', "-");

        Ok(companies
            .iter()
            .find(|company| {
                company.ticker.eq_ignore_ascii_case(ticker)
                    || company.ticker.eq_ignore_ascii_case(&edgar_ticker)
            })
            .cloned())
    }

    async fn fetch_ticker_index(&self) -> Result<Vec<SecTickerCompany>, AppError> {
        self.throttle
            .reserve("sec_edgar", "company_tickers", 1)
            .await?;
        let response = self
            .client
            .get("https://www.sec.gov/files/company_tickers.json")
            .send()
            .await?
            .error_for_status()?;
        let companies = response
            .json::<HashMap<String, SecTickerCompany>>()
            .await?
            .into_values()
            .collect();

        Ok(companies)
    }
}

pub struct EdgarReportEvent {
    pub ticker: String,
    pub date: NaiveDate,
    pub form: String,
    pub filing_date: Option<NaiveDate>,
}

#[derive(Clone, Deserialize)]
struct SecTickerCompany {
    cik_str: u64,
    ticker: String,
    title: String,
}

#[derive(Deserialize)]
struct SecCompanyFacts {
    #[serde(default)]
    facts: HashMap<String, HashMap<String, SecFactConcept>>,
}

#[derive(Deserialize)]
struct SecFactConcept {
    #[serde(default)]
    units: HashMap<String, Vec<SecFactUnit>>,
}

#[derive(Clone, Deserialize)]
struct SecFactUnit {
    end: Option<String>,
    val: Option<f64>,
    fy: Option<i32>,
    fp: Option<String>,
    form: Option<String>,
    filed: Option<String>,
}

#[derive(Deserialize)]
struct SecSubmissions {
    name: Option<String>,
    #[serde(rename = "entityType")]
    entity_type: Option<String>,
    sic: Option<String>,
    #[serde(rename = "sicDescription")]
    sic_description: Option<String>,
    tickers: Option<Vec<String>>,
    exchanges: Option<Vec<String>>,
    #[serde(rename = "fiscalYearEnd")]
    fiscal_year_end: Option<String>,
    #[serde(rename = "stateOfIncorporation")]
    state_of_incorporation: Option<String>,
    #[serde(rename = "stateOfIncorporationDescription")]
    state_of_incorporation_description: Option<String>,
    phone: Option<String>,
    #[serde(default)]
    addresses: SecAddresses,
    #[serde(default)]
    filings: SecFilings,
}

impl SecSubmissions {
    fn recent_filings(&self, cik: &str, limit: usize) -> Vec<CompanyFiling> {
        let cik_number = cik.trim_start_matches('0');
        (0..self.filings.recent.form.len().min(limit))
            .map(|index| {
                let accession_number = optional_at(&self.filings.recent.accession_number, index);
                let primary_document = optional_at(&self.filings.recent.primary_document, index);
                let url = filing_url(
                    cik_number,
                    accession_number.as_deref(),
                    primary_document.as_deref(),
                );

                CompanyFiling {
                    form: self.filings.recent.form[index].clone(),
                    filing_date: optional_at(&self.filings.recent.filing_date, index),
                    report_date: optional_at(&self.filings.recent.report_date, index),
                    accession_number,
                    primary_document,
                    description: optional_at(&self.filings.recent.primary_doc_description, index),
                    url,
                }
            })
            .collect()
    }

    fn report_events(&self, ticker: &str, start_date: NaiveDate) -> Vec<EdgarReportEvent> {
        (0..self.filings.recent.form.len())
            .filter_map(|index| {
                let form = self.filings.recent.form[index].as_str();
                if !matches!(form, "10-K" | "10-Q") {
                    return None;
                }

                let date = optional_at(&self.filings.recent.report_date, index)
                    .and_then(|date| parse_naive_date(&date))?;
                if date < start_date {
                    return None;
                }

                Some(EdgarReportEvent {
                    ticker: ticker.to_string(),
                    date,
                    form: form.to_string(),
                    filing_date: optional_at(&self.filings.recent.filing_date, index)
                        .and_then(|date| parse_naive_date(&date)),
                })
            })
            .collect()
    }
}

#[derive(Default, Deserialize)]
struct SecAddresses {
    business: Option<SecAddress>,
    mailing: Option<SecAddress>,
}

#[derive(Default, Deserialize)]
struct SecFilings {
    #[serde(default)]
    recent: SecRecentFilings,
}

#[derive(Default, Deserialize)]
struct SecRecentFilings {
    #[serde(rename = "accessionNumber", default)]
    accession_number: Vec<String>,
    #[serde(rename = "filingDate", default)]
    filing_date: Vec<String>,
    #[serde(rename = "reportDate", default)]
    report_date: Vec<String>,
    #[serde(default)]
    form: Vec<String>,
    #[serde(rename = "primaryDocument", default)]
    primary_document: Vec<String>,
    #[serde(rename = "primaryDocDescription", default)]
    primary_doc_description: Vec<String>,
}

#[derive(Deserialize)]
struct SecAddress {
    street1: Option<String>,
    street2: Option<String>,
    city: Option<String>,
    #[serde(rename = "stateOrCountryDescription")]
    state_or_country_description: Option<String>,
    #[serde(rename = "stateOrCountry")]
    state_or_country: Option<String>,
    #[serde(rename = "zipCode")]
    zip_code: Option<String>,
}

impl From<SecAddress> for CompanyAddress {
    fn from(value: SecAddress) -> Self {
        Self {
            street1: non_empty(value.street1),
            street2: non_empty(value.street2),
            city: non_empty(value.city),
            state_or_country: non_empty(value.state_or_country_description)
                .or_else(|| non_empty(value.state_or_country)),
            zip_code: non_empty(value.zip_code),
        }
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn optional_at(values: &[String], index: usize) -> Option<String> {
    values
        .get(index)
        .cloned()
        .and_then(|value| non_empty(Some(value)))
}

fn parse_naive_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn filing_url(
    cik: &str,
    accession_number: Option<&str>,
    primary_document: Option<&str>,
) -> Option<String> {
    let accession_number = accession_number?;
    let primary_document = primary_document?;
    let accession_path = accession_number.replace('-', "");

    Some(format!(
        "https://www.sec.gov/Archives/edgar/data/{cik}/{accession_path}/{primary_document}"
    ))
}

fn financial_metrics(facts: &SecCompanyFacts) -> Vec<CompanyFinancialMetric> {
    let mut metrics = financial_specs()
        .into_iter()
        .filter_map(|spec| metric_for_spec(facts, spec))
        .collect::<Vec<_>>();

    if let Some(metric) = calculated_free_cash_flow(&metrics) {
        metrics.push(metric);
    }

    if let Some(metric) = calculated_total_debt(&metrics) {
        metrics.push(metric);
    }

    metrics
}

fn metric_for_spec(
    facts: &SecCompanyFacts,
    spec: FinancialMetricSpec,
) -> Option<CompanyFinancialMetric> {
    spec.concepts.iter().find_map(|concept_ref| {
        if let Some((unit, fact)) = best_fact(
            facts,
            concept_ref.taxonomy,
            concept_ref.concept,
            spec.unit_priority,
        ) {
            Some(CompanyFinancialMetric {
                key: spec.key.to_string(),
                label: spec.label.to_string(),
                value: fact.val?,
                unit: unit.to_string(),
                fiscal_year: fact.fy,
                fiscal_period: fact.fp.clone(),
                form: fact.form.clone(),
                filed: fact.filed.clone(),
                end: fact.end.clone(),
                concept: concept_ref.concept.to_string(),
            })
        } else {
            None
        }
    })
}

fn best_fact<'a>(
    facts: &'a SecCompanyFacts,
    taxonomy: &str,
    concept: &str,
    unit_priority: &[&str],
) -> Option<(&'a str, &'a SecFactUnit)> {
    let concept = facts.facts.get(taxonomy)?.get(concept)?;

    unit_priority
        .iter()
        .filter_map(|unit| concept.units.get_key_value(*unit))
        .find_map(|(unit, values)| {
            values
                .iter()
                .filter(|fact| fact.val.is_some())
                .max_by_key(|fact| fact_score(fact))
                .map(|fact| (unit.as_str(), fact))
        })
}

fn fact_score(fact: &SecFactUnit) -> (u8, String, String) {
    let annual_score = match fact.form.as_deref() {
        Some("10-K") | Some("20-F") | Some("40-F") => 3,
        Some("10-Q") => 2,
        Some(_) => 1,
        None => 0,
    };

    (
        annual_score,
        fact.filed.clone().unwrap_or_default(),
        fact.end.clone().unwrap_or_default(),
    )
}

fn calculated_free_cash_flow(metrics: &[CompanyFinancialMetric]) -> Option<CompanyFinancialMetric> {
    let operating_cash_flow = metrics
        .iter()
        .find(|metric| metric.key == "operating_cash_flow")?;
    let capex = metrics.iter().find(|metric| metric.key == "capex")?;

    if operating_cash_flow.unit != capex.unit || operating_cash_flow.end != capex.end {
        return None;
    }

    Some(CompanyFinancialMetric {
        key: "free_cash_flow".to_string(),
        label: "Free cash flow".to_string(),
        value: operating_cash_flow.value - capex.value.abs(),
        unit: operating_cash_flow.unit.clone(),
        fiscal_year: operating_cash_flow.fiscal_year,
        fiscal_period: operating_cash_flow.fiscal_period.clone(),
        form: operating_cash_flow.form.clone(),
        filed: operating_cash_flow.filed.clone(),
        end: operating_cash_flow.end.clone(),
        concept: "calculated: operating cash flow - capex".to_string(),
    })
}

fn calculated_total_debt(metrics: &[CompanyFinancialMetric]) -> Option<CompanyFinancialMetric> {
    let current_debt = metrics.iter().find(|metric| metric.key == "current_debt")?;
    let long_term_debt = metrics
        .iter()
        .find(|metric| metric.key == "long_term_debt")?;

    if current_debt.unit != long_term_debt.unit || current_debt.end != long_term_debt.end {
        return None;
    }

    Some(CompanyFinancialMetric {
        key: "total_debt".to_string(),
        label: "Total debt".to_string(),
        value: current_debt.value + long_term_debt.value,
        unit: current_debt.unit.clone(),
        fiscal_year: current_debt.fiscal_year,
        fiscal_period: current_debt.fiscal_period.clone(),
        form: current_debt.form.clone(),
        filed: current_debt.filed.clone(),
        end: current_debt.end.clone(),
        concept: "calculated: current debt + long-term debt".to_string(),
    })
}

fn metric_i64(metrics: &[CompanyFinancialMetric], key: &str) -> Option<i64> {
    metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| {
            if metric.value.is_finite() {
                Some(metric.value.round() as i64)
            } else {
                None
            }
        })
}

fn metric_usd_cents(metrics: &[CompanyFinancialMetric], key: &str) -> Option<i64> {
    metrics
        .iter()
        .find(|metric| metric.key == key && metric.unit == "USD")
        .and_then(|metric| {
            if metric.value.is_finite() {
                Some((metric.value * 100.0).round() as i64)
            } else {
                None
            }
        })
}

struct FinancialMetricSpec {
    key: &'static str,
    label: &'static str,
    unit_priority: &'static [&'static str],
    concepts: &'static [ConceptRef],
}

struct ConceptRef {
    taxonomy: &'static str,
    concept: &'static str,
}

const USD_UNITS: &[&str] = &["USD"];
const EPS_UNITS: &[&str] = &["USD/shares"];
const SHARE_UNITS: &[&str] = &["shares"];

fn financial_specs() -> Vec<FinancialMetricSpec> {
    vec![
        FinancialMetricSpec {
            key: "revenue",
            label: "Revenue",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "RevenueFromContractWithCustomerExcludingAssessedTax",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "Revenues",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "SalesRevenueNet",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "SalesRevenueGoodsNet",
                },
            ],
        },
        FinancialMetricSpec {
            key: "gross_profit",
            label: "Gross profit",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "GrossProfit",
            }],
        },
        FinancialMetricSpec {
            key: "operating_income",
            label: "Operating income",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "OperatingIncomeLoss",
            }],
        },
        FinancialMetricSpec {
            key: "net_income",
            label: "Net income",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "NetIncomeLoss",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "ProfitLoss",
                },
            ],
        },
        FinancialMetricSpec {
            key: "eps_basic",
            label: "EPS basic",
            unit_priority: EPS_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "EarningsPerShareBasic",
            }],
        },
        FinancialMetricSpec {
            key: "eps_diluted",
            label: "EPS diluted",
            unit_priority: EPS_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "EarningsPerShareDiluted",
            }],
        },
        FinancialMetricSpec {
            key: "assets",
            label: "Assets",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "Assets",
            }],
        },
        FinancialMetricSpec {
            key: "liabilities",
            label: "Liabilities",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "Liabilities",
            }],
        },
        FinancialMetricSpec {
            key: "equity",
            label: "Equity",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "StockholdersEquity",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "StockholdersEquityIncludingPortionAttributableToNoncontrollingInterest",
                },
            ],
        },
        FinancialMetricSpec {
            key: "cash",
            label: "Cash",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "CashAndCashEquivalentsAtCarryingValue",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "CashCashEquivalentsRestrictedCashAndRestrictedCashEquivalents",
                },
            ],
        },
        FinancialMetricSpec {
            key: "operating_cash_flow",
            label: "Operating cash flow",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "NetCashProvidedByUsedInOperatingActivities",
            }],
        },
        FinancialMetricSpec {
            key: "capex",
            label: "Capex",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "PaymentsToAcquirePropertyPlantAndEquipment",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "PaymentsToAcquireProductiveAssets",
                },
            ],
        },
        FinancialMetricSpec {
            key: "shares_outstanding",
            label: "Shares outstanding",
            unit_priority: SHARE_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "dei",
                concept: "EntityCommonStockSharesOutstanding",
            }],
        },
        FinancialMetricSpec {
            key: "current_debt",
            label: "Current debt",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "LongTermDebtAndFinanceLeaseObligationsCurrent",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "LongTermDebtCurrent",
                },
            ],
        },
        FinancialMetricSpec {
            key: "long_term_debt",
            label: "Long-term debt",
            unit_priority: USD_UNITS,
            concepts: &[
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "LongTermDebtAndFinanceLeaseObligationsNoncurrent",
                },
                ConceptRef {
                    taxonomy: "us-gaap",
                    concept: "LongTermDebtNoncurrent",
                },
            ],
        },
        FinancialMetricSpec {
            key: "current_assets",
            label: "Current assets",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "AssetsCurrent",
            }],
        },
        FinancialMetricSpec {
            key: "current_liabilities",
            label: "Current liabilities",
            unit_priority: USD_UNITS,
            concepts: &[ConceptRef {
                taxonomy: "us-gaap",
                concept: "LiabilitiesCurrent",
            }],
        },
    ]
}
