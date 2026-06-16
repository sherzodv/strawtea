use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone, Utc};
use lopdf::Document;
use sha2::{Digest, Sha256};

use crate::error::AppError;

pub const OPTIMA_PARSER_NAME: &str = "optima_pdf_statement";
pub const OPTIMA_PARSER_VERSION: i32 = 2;

#[derive(Debug)]
pub struct ParsedStatement {
    pub bank: String,
    pub account_number_masked: Option<String>,
    pub card_number_masked: Option<String>,
    pub account_currency: String,
    pub statement_period_start: Option<NaiveDate>,
    pub statement_period_end: Option<NaiveDate>,
    pub rows: Vec<ParsedRawtx>,
}

#[derive(Debug)]
pub struct ParsedRawtx {
    pub row_index: i32,
    pub occurred_at: DateTime<Utc>,
    pub posted_date: NaiveDate,
    pub description_raw: String,
    pub row_raw: String,
    pub operation_amount: i64,
    pub operation_currency: String,
    pub fee_amount: i64,
    pub fee_currency: String,
    pub account_amount: Option<i64>,
    pub account_amount_currency: Option<String>,
    pub direction: String,
    pub raw_kind: Option<String>,
    pub semantic_key_base: String,
    pub semantic_ordinal: i32,
    pub dedupe_key: String,
    pub raw_fingerprint: String,
}

#[derive(Clone, Debug)]
struct Money {
    amount: i64,
    currency: String,
    line_index: usize,
    start: usize,
}

pub fn parse_optima_pdf(bytes: &[u8]) -> Result<ParsedStatement, AppError> {
    let text = extract_pdf_text(bytes)?;
    parse_optima_text(&text)
}

fn extract_pdf_text(bytes: &[u8]) -> Result<String, AppError> {
    let document = Document::load_mem(bytes)
        .map_err(|err| AppError::BadRequest(format!("could not read PDF: {err}")))?;
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    document
        .extract_text(&pages)
        .map_err(|err| AppError::BadRequest(format!("could not extract PDF text: {err}")))
}

fn parse_optima_text(text: &str) -> Result<ParsedStatement, AppError> {
    let lines: Vec<String> = text.lines().map(clean_line).collect();
    let account_number_masked =
        header_value(&lines, "Номер счета:").or_else(|| tokenized_account_number(&lines));
    let card_number_masked =
        header_value(&lines, "Номер карты:").or_else(|| tokenized_card_number(&lines));
    let account_currency = header_value(&lines, "Валюта счета:")
        .or_else(|| tokenized_account_currency(&lines))
        .unwrap_or_else(|| "KGS".to_string())
        .trim()
        .to_string();
    let (statement_period_start, statement_period_end) = header_value(&lines, "Период:")
        .map(|value| parse_period(&value))
        .or_else(|| tokenized_period(&lines))
        .unwrap_or((None, None));

    let blocks = transaction_blocks(&lines);
    if blocks.is_empty() {
        return Err(AppError::BadRequest(
            "no transactions found in statement".to_string(),
        ));
    }

    let mut rows = Vec::with_capacity(blocks.len());
    let mut ordinals: HashMap<String, i32> = HashMap::new();

    let account_identity = account_number_masked
        .clone()
        .or_else(|| card_number_masked.clone())
        .unwrap_or_else(|| "unknown".to_string());

    for (index, block) in blocks.into_iter().enumerate() {
        let mut row = parse_block(
            index as i32 + 1,
            &block,
            &account_currency,
            &account_identity,
        )?;
        let ordinal = ordinals
            .entry(row.semantic_key_base.clone())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        row.semantic_ordinal = *ordinal;
        row.dedupe_key = sha256_hex(format!(
            "{}|{}",
            row.semantic_key_base, row.semantic_ordinal
        ));
        rows.push(row);
    }

    Ok(ParsedStatement {
        bank: "optima".to_string(),
        account_number_masked,
        card_number_masked,
        account_currency,
        statement_period_start,
        statement_period_end,
        rows,
    })
}

fn parse_block(
    row_index: i32,
    block: &[String],
    account_currency: &str,
    account_identity: &str,
) -> Result<ParsedRawtx, AppError> {
    let first_line = block
        .first()
        .ok_or_else(|| AppError::BadRequest("empty transaction block".to_string()))?;
    let first_trimmed = first_line.trim();
    let date_text = first_trimmed
        .get(0..10)
        .ok_or_else(|| AppError::BadRequest("transaction date is missing".to_string()))?;
    let posted_date = parse_date(date_text)?;
    let time = block
        .iter()
        .find_map(|line| parse_time(line.trim()))
        .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).expect("valid midnight"));
    let local_offset = FixedOffset::east_opt(6 * 60 * 60).expect("valid KGT offset");
    let occurred_at = local_offset
        .from_local_datetime(&posted_date.and_time(time))
        .single()
        .ok_or_else(|| AppError::BadRequest("invalid transaction timestamp".to_string()))?
        .with_timezone(&Utc);

    let money_values = money_in_block(block);
    let operation = money_values.first().ok_or_else(|| {
        AppError::BadRequest(format!(
            "amount is missing for row {row_index}: {}",
            block.join(" | ")
        ))
    })?;
    let fee = money_values.get(1).cloned().unwrap_or_else(|| Money {
        amount: 0,
        currency: account_currency.to_string(),
        line_index: operation.line_index,
        start: usize::MAX,
    });
    let conversion = money_values.get(2);

    let mut description_parts = Vec::new();
    let first_without_date = first_trimmed.get(10..).unwrap_or("").trim();
    let description_head = if operation.line_index == 0 && operation.start != usize::MAX {
        first_trimmed
            .get(10..operation.start)
            .unwrap_or(first_without_date)
            .trim()
    } else {
        first_without_date
    };
    if !description_head.is_empty() {
        description_parts.push(description_head.to_string());
    }
    for line in block.iter().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || parse_time(trimmed).is_some()
            || is_page_marker(trimmed)
            || is_amount_token_line(trimmed)
            || is_money_only_line(trimmed)
            || is_amount_and_fee_line(trimmed)
            || is_currency_line(trimmed)
        {
            continue;
        }
        description_parts.push(trimmed.to_string());
    }
    let description_raw = description_parts.join(" ").trim().to_string();
    let row_raw = block.join("\n");
    let direction = direction(operation.amount, fee.amount).to_string();
    let raw_kind = raw_kind(&description_raw, operation.amount, fee.amount).map(str::to_string);
    let account_amount = conversion.map(|money| money.amount);
    let account_amount_currency = conversion.map(|money| money.currency.clone());

    let semantic_key_base = [
        "optima".to_string(),
        account_identity.to_string(),
        occurred_at.to_rfc3339(),
        operation.amount.to_string(),
        operation.currency.clone(),
        fee.amount.to_string(),
        fee.currency.clone(),
        account_amount
            .map(|value| value.to_string())
            .unwrap_or_default(),
        account_amount_currency.clone().unwrap_or_default(),
    ]
    .join("|");

    Ok(ParsedRawtx {
        row_index,
        occurred_at,
        posted_date,
        description_raw,
        row_raw: row_raw.clone(),
        operation_amount: operation.amount,
        operation_currency: operation.currency.clone(),
        fee_amount: fee.amount,
        fee_currency: fee.currency.clone(),
        account_amount,
        account_amount_currency,
        direction,
        raw_kind,
        semantic_key_base,
        semantic_ordinal: 0,
        dedupe_key: String::new(),
        raw_fingerprint: sha256_hex(row_raw),
    })
}

fn transaction_blocks(lines: &[String]) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut in_transactions = false;

    for line in lines {
        let trimmed = line.trim();
        if !in_transactions {
            if trimmed.contains("Детали операции")
                || trimmed.contains("Сумма операции")
                || trimmed == "Комиссия"
            {
                in_transactions = true;
            }
            continue;
        }

        if is_date_line(trimmed) {
            if !current.is_empty() {
                if !money_in_block(&current).is_empty() {
                    blocks.push(current);
                }
                current = Vec::new();
            }
            current.push(trimmed.to_string());
        } else if !current.is_empty() {
            if is_page_marker(trimmed) {
                continue;
            }
            current.push(trimmed.to_string());
        }
    }

    if !current.is_empty() {
        if !money_in_block(&current).is_empty() {
            blocks.push(current);
        }
    }

    blocks
}

fn money_spans(line: &str) -> Vec<Money> {
    let tokens = token_positions(line);
    let mut money = Vec::new();

    for window in tokens.windows(2) {
        let (amount_text, start, _) = window[0];
        let (currency, _, end) = window[1];
        if !matches!(currency, "KGS" | "USD" | "EUR") {
            continue;
        }
        if let Some(amount) = parse_minor_units(amount_text) {
            money.push(Money {
                amount,
                currency: currency.to_string(),
                line_index: 0,
                start,
            });
        }
        if end >= line.len() {
            break;
        }
    }

    money
}

fn money_in_block(block: &[String]) -> Vec<Money> {
    let mut tokens = Vec::new();

    for (line_index, line) in block.iter().enumerate() {
        for (token, start, _end) in token_positions(line) {
            tokens.push((token.to_string(), line_index, start));
        }
    }

    let mut money = Vec::new();
    for window in tokens.windows(2) {
        let (amount_text, line_index, start) = &window[0];
        let (currency, _, _) = &window[1];
        if !matches!(currency.as_str(), "KGS" | "USD" | "EUR") {
            continue;
        }
        if let Some(amount) = parse_minor_units(amount_text) {
            money.push(Money {
                amount,
                currency: currency.clone(),
                line_index: *line_index,
                start: *start,
            });
        }
    }

    money
}

fn token_positions(line: &str) -> Vec<(&str, usize, usize)> {
    let mut tokens = Vec::new();
    let mut start = None;

    for (index, char) in line.char_indices() {
        if char.is_whitespace() {
            if let Some(token_start) = start.take() {
                tokens.push((&line[token_start..index], token_start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }

    if let Some(token_start) = start {
        tokens.push((&line[token_start..], token_start, line.len()));
    }

    tokens
}

fn parse_minor_units(value: &str) -> Option<i64> {
    let value = value.replace(',', ".");
    let negative = value.starts_with('-');
    let normalized = value.trim_start_matches(['+', '-']);
    let mut parts = normalized.split('.');
    let whole = parts.next()?.parse::<i64>().ok()?;
    let fraction = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return None;
    }
    let mut cents = fraction.chars().take(2).collect::<String>();
    while cents.len() < 2 {
        cents.push('0');
    }
    let cents = cents.parse::<i64>().ok()?;
    let amount = whole * 100 + cents;
    Some(if negative { -amount } else { amount })
}

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%d.%m.%Y")
        .map_err(|_| AppError::BadRequest(format!("invalid date: {value}")))
}

fn parse_time(value: &str) -> Option<NaiveTime> {
    if value.len() != 5 || !value.as_bytes().get(2).is_some_and(|byte| *byte == b':') {
        return None;
    }
    NaiveTime::parse_from_str(value, "%H:%M").ok()
}

fn parse_period(value: &str) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let mut parts = value.split('-').map(str::trim);
    let start = parts.next().and_then(|part| parse_date(part).ok());
    let end = parts.next().and_then(|part| parse_date(part).ok());
    (start, end)
}

fn header_value(lines: &[String], label: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(label)
            .map(|value| value.trim().to_string())
    })
}

fn tokenized_card_number(lines: &[String]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "карты:" {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|prev| lines.get(prev))?;
        if previous.trim() != "Номер" {
            continue;
        }
        let first = lines.get(index + 1)?.trim();
        let second = lines.get(index + 2)?.trim();
        return Some(format!("{first}{second}"));
    }
    None
}

fn tokenized_account_number(lines: &[String]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "счета:" {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|prev| lines.get(prev))?;
        if previous.trim() != "Номер" {
            continue;
        }
        return lines.get(index + 1).map(|value| value.trim().to_string());
    }
    None
}

fn tokenized_account_currency(lines: &[String]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "счета:" {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|prev| lines.get(prev))?;
        if previous.trim() != "Валюта" {
            continue;
        }
        return lines.get(index + 1).map(|value| value.trim().to_string());
    }
    None
}

fn tokenized_period(lines: &[String]) -> Option<(Option<NaiveDate>, Option<NaiveDate>)> {
    let period_index = lines.iter().position(|line| line.trim() == "Период:")?;
    let dates: Vec<NaiveDate> = lines
        .iter()
        .skip(period_index + 1)
        .filter_map(|line| {
            line.trim()
                .get(0..10)
                .and_then(|value| parse_date(value).ok())
        })
        .take(2)
        .collect();

    Some((dates.first().copied(), dates.get(1).copied()))
}

fn is_date_line(value: &str) -> bool {
    value
        .get(0..10)
        .is_some_and(|prefix| NaiveDate::parse_from_str(prefix, "%d.%m.%Y").is_ok())
}

fn is_page_marker(value: &str) -> bool {
    let Some((left, right)) = value.split_once('/') else {
        return false;
    };
    left.trim().parse::<u32>().is_ok() && right.trim().parse::<u32>().is_ok()
}

fn is_money_only_line(value: &str) -> bool {
    let money = money_spans(value);
    money.len() == 1 && value.starts_with(|char: char| char.is_ascii_digit() || char == '-')
}

fn is_amount_and_fee_line(value: &str) -> bool {
    money_spans(value).len() >= 2
}

fn is_amount_token_line(value: &str) -> bool {
    parse_minor_units(value).is_some()
}

fn is_currency_line(value: &str) -> bool {
    matches!(value, "KGS" | "USD" | "EUR")
}

fn direction(operation_amount: i64, fee_amount: i64) -> &'static str {
    if operation_amount < 0 || fee_amount < 0 {
        "debit"
    } else if operation_amount > 0 {
        "credit"
    } else {
        "neutral"
    }
}

fn raw_kind(description: &str, operation_amount: i64, fee_amount: i64) -> Option<&'static str> {
    let lower = description.to_lowercase();

    if lower.contains("regular charge") || (operation_amount == 0 && fee_amount < 0) {
        Some("fee")
    } else if lower.contains("возврат") {
        Some("refund")
    } else if lower.contains("наличное списание") {
        Some("cash_withdrawal")
    } else if lower.contains("перевод с карты") || lower.contains("перевод на visa")
    {
        Some("transfer")
    } else if operation_amount < 0 {
        Some("spend")
    } else if operation_amount > 0 {
        Some("income")
    } else {
        None
    }
}

fn clean_line(line: &str) -> String {
    line.trim_start_matches('\u{c}').to_string()
}

pub fn sha256_hex(value: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(value.as_ref());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::parse_optima_pdf;

    #[test]
    fn sample_optima_pdfs_parse() {
        let samples = [
            (
                "../sample-data/optima-bcard-010125-140626.pdf",
                1416,
                "KGS",
                "4169-61xx-xxxx-9250",
            ),
            (
                "../sample-data/optima-som-card-010124-140626.pdf",
                736,
                "KGS",
                "4169-58xx-xxxx-7472",
            ),
            (
                "../sample-data/optima-usd-card-010124-140626.pdf",
                39,
                "USD",
                "4169-58xx-xxxx-4850",
            ),
        ];

        for (sample, expected_rows, expected_currency, expected_card) in samples {
            if !Path::new(sample).exists() {
                eprintln!("skipping missing sample {sample}");
                continue;
            }

            let bytes = fs::read(sample).expect("sample PDF should be readable");
            let parsed = match parse_optima_pdf(&bytes) {
                Ok(parsed) => parsed,
                Err(err) => {
                    let text = super::extract_pdf_text(&bytes).expect("sample PDF text");
                    for (index, line) in text.lines().take(120).enumerate() {
                        eprintln!("{:03}: {}", index + 1, line);
                    }
                    panic!("sample PDF should parse: {err:?}");
                }
            };
            eprintln!(
                "{sample}: {} rows, account {}, card {:?}",
                parsed.rows.len(),
                parsed.account_currency,
                parsed.card_number_masked
            );
            assert_eq!(parsed.rows.len(), expected_rows, "{sample} row count");
            assert_eq!(
                parsed.account_currency, expected_currency,
                "{sample} currency"
            );
            assert_eq!(
                parsed.card_number_masked.as_deref(),
                Some(expected_card),
                "{sample} card"
            );
        }
    }
}
