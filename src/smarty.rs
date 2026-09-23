use crate::atmb::model::Address;
use color_eyre::eyre::{bail, eyre};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct SmartyClient {
    client: Client,
    credentials: Vec<(String, String)>,
    current: usize,
}
#[derive(Debug, Default)]
pub struct AdditionalInfo {
    pub status: String,
    pub cmra: String,
    pub rdi: String,
}
impl SmartyClient {
    pub fn new() -> color_eyre::Result<Self> {
        let raw = std::env::var("CREDENTIALS").unwrap_or_default();
        let credentials = if !raw.trim().is_empty() {
            parse_credentials(&raw)?
        } else {
            let id = std::env::var("SMARTY_AUTH_ID").unwrap_or_default();
            let token = std::env::var("SMARTY_AUTH_TOKEN").unwrap_or_default();
            if id.trim().is_empty() || token.trim().is_empty() {
                bail!("Set CREDENTIALS or both SMARTY_AUTH_ID and SMARTY_AUTH_TOKEN");
            }
            vec![(id.trim().to_owned(), token.trim().to_owned())]
        };
        Ok(Self {
            client: Client::builder().timeout(Duration::from_secs(30)).build()?,
            credentials,
            current: 0,
        })
    }
    pub async fn inquire(&mut self, address: &Address) -> AdditionalInfo {
        while self.current < self.credentials.len() {
            let (id, token) = &self.credentials[self.current];
            for attempt in 0..3 {
                let response = self
                    .client
                    .get("https://us-street.api.smarty.com/street-address")
                    .query(&[
                        ("auth-id", id.as_str()),
                        ("auth-token", token.as_str()),
                        ("street", &address.line1),
                        ("street2", &address.line2),
                        ("city", &address.city),
                        ("state", &address.state),
                        ("zipcode", &address.zip),
                        ("match", "enhanced"),
                        ("candidates", "1"),
                    ])
                    .send()
                    .await;
                match response {
                    Ok(response) => {
                        let status = response.status();
                        if status.as_u16() == 402 {
                            self.current += 1;
                            break;
                        }
                        if (status.is_server_error() || status.as_u16() == 429) && attempt < 2 {
                            tokio::time::sleep(Duration::from_secs(2 * (attempt + 1))).await;
                            continue;
                        }
                        if !status.is_success() {
                            return AdditionalInfo::error(format!("http_{}", status.as_u16()));
                        }
                        return match response.json::<Value>().await {
                            Ok(value) => parse_response(value),
                            Err(_) => AdditionalInfo::error("invalid_json"),
                        };
                    }
                    Err(_) if attempt < 2 => {
                        tokio::time::sleep(Duration::from_secs(2 * (attempt + 1))).await
                    }
                    Err(_) => return AdditionalInfo::error("network_error"),
                }
            }
        }
        AdditionalInfo::error("quota_exhausted")
    }
}
impl AdditionalInfo {
    pub fn error(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            ..Self::default()
        }
    }
    pub fn failed(&self) -> bool {
        !matches!(self.status.as_str(), "matched" | "no_match")
    }
}
fn parse_credentials(raw: &str) -> color_eyre::Result<Vec<(String, String)>> {
    raw.split(',')
        .map(|pair| {
            let (id, token) = pair.trim().split_once('=').ok_or_else(|| {
                eyre!("CREDENTIALS must contain ID=TOKEN pairs separated by commas")
            })?;
            if id.trim().is_empty() || token.trim().is_empty() {
                bail!("CREDENTIALS contains an empty ID or TOKEN");
            }
            Ok((id.trim().into(), token.trim().into()))
        })
        .collect()
}
fn parse_response(value: Value) -> AdditionalInfo {
    let Some(candidates) = value.as_array() else {
        return AdditionalInfo::error("invalid_response");
    };
    let Some(candidate) = candidates.first() else {
        return AdditionalInfo::error("no_match");
    };
    AdditionalInfo {
        status: "matched".into(),
        cmra: candidate
            .pointer("/analysis/dpv_cmra")
            .and_then(Value::as_str)
            .filter(|v| matches!(*v, "Y" | "N"))
            .unwrap_or("Unknown")
            .into(),
        rdi: candidate
            .pointer("/metadata/rdi")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or("Unknown")
            .into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_validated_without_echoing_secrets() {
        assert_eq!(parse_credentials(" a=b, c=d ").unwrap().len(), 2);
        for raw in ["", "sensitive-value", "a=", "=b", "a=b,"] {
            let message = parse_credentials(raw).unwrap_err().to_string();
            assert!(!message.contains("sensitive-value"));
        }
    }
    #[test]
    fn unknown_is_not_non_cmra() {
        let result = parse_response(serde_json::json!([{"analysis": {}, "metadata": {}}]));
        assert_eq!(result.cmra, "Unknown");
        assert_eq!(parse_response(serde_json::json!([])).status, "no_match");
        assert!(parse_response(serde_json::json!({"error":"bad"})).failed());
    }
}
