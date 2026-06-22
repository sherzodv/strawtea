use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone)]
pub struct OpenAiClient {
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiCompanyContext {
    pub ticker: String,
    pub name: String,
    pub sic_description: Option<String>,
    pub deterministic_tier: Option<String>,
    pub deterministic_score: i32,
    pub theme_matches: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiEnrichment {
    pub tier: Option<String>,
    pub score_adjustment: i32,
    pub ai_relevance_reasons: Vec<String>,
    pub confidence: f64,
    pub warnings: Vec<String>,
}

impl OpenAiClient {
    pub fn new(api_key: String, model: String) -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|err| AppError::Ai(err.to_string()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self { model, client })
    }

    pub async fn enrich_ai_relevance(
        &self,
        context: &AiCompanyContext,
    ) -> Result<AiEnrichment, AppError> {
        let input = format!(
            "{}\n\nCompany context JSON:\n{}",
            AI_RELEVANCE_PROMPT,
            serde_json::to_string(context).map_err(|err| AppError::Ai(err.to_string()))?
        );
        let payload = serde_json::json!({
            "model": self.model,
            "input": input,
            "max_output_tokens": 700
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAiResponse>()
            .await?;

        let text = response
            .output_text()
            .ok_or_else(|| AppError::Ai("response did not include text output".to_string()))?;
        let json_text = strip_json_fence(text.trim());
        serde_json::from_str::<AiEnrichment>(json_text)
            .map_err(|err| AppError::Ai(format!("invalid AI JSON: {err}")))
    }
}

const AI_RELEVANCE_PROMPT: &str = r#"Classify whether this public company is meaningfully connected to the AI growth cycle.

Use only the provided stable company context. Do not use news, rumors, stock price action, or trading recommendations.

Return only JSON with this exact shape:
{
  "tier": "1" | "2" | "3" | null,
  "score_adjustment": integer from -15 to 15,
  "ai_relevance_reasons": ["short factual reason", "..."],
  "confidence": number from 0 to 1,
  "warnings": ["short warning", "..."]
}

Tier 1 is direct AI platforms, chips, cloud, or accelerated computing.
Tier 2 is AI infrastructure suppliers such as semiconductor equipment, memory, networking, EDA, optical interconnect, and data center hardware.
Tier 3 is indirect physical or industrial beneficiaries such as power, cooling, electrical equipment, automation, testing, specialty materials, and electronics supply chain.

Do not reward generic AI marketing language. If the company context is too weak, use null tier and a negative adjustment."#;

#[derive(Deserialize)]
struct OpenAiResponse {
    output: Vec<OpenAiOutput>,
}

#[derive(Deserialize)]
struct OpenAiOutput {
    content: Option<Vec<OpenAiContent>>,
}

#[derive(Deserialize)]
struct OpenAiContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

impl OpenAiResponse {
    fn output_text(&self) -> Option<&str> {
        self.output.iter().find_map(|output| {
            output.content.as_ref()?.iter().find_map(|content| {
                if content.content_type == "output_text" || content.content_type == "text" {
                    content.text.as_deref()
                } else {
                    None
                }
            })
        })
    }
}

fn strip_json_fence(value: &str) -> &str {
    value
        .strip_prefix("```json")
        .or_else(|| value.strip_prefix("```"))
        .and_then(|stripped| stripped.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(value)
}
