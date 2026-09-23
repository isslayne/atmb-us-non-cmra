use crate::{atmb::model::Address, config::state_code};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

const ENDPOINT: &str = "https://tools.usps.com/tools/app/ziplookup/zipByAddress";
#[derive(Debug, Default)]
pub struct UspsResult {
    pub status: String,
    pub address: String,
    pub zip5: String,
    pub zip4: String,
    pub dpv_confirmation: String,
    pub cmra: String,
    pub business: String,
    pub carrier_route: String,
    pub raw: String,
}
impl UspsResult {
    pub fn status(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            ..Self::default()
        }
    }
    pub fn service_error(&self) -> bool {
        !matches!(
            self.status.as_str(),
            "matched" | "no_match" | "multiple_matches" | "disabled"
        )
    }
}
pub struct UspsClient {
    client: Client,
    interval: Duration,
    consecutive_errors: usize,
}
impl UspsClient {
    pub fn new(interval_ms: usize) -> color_eyre::Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Mozilla/5.0 (compatible; ATMB-address-check/1.0)")
                .build()?,
            interval: Duration::from_millis(interval_ms as u64),
            consecutive_errors: 0,
        })
    }
    pub async fn inquire(&mut self, address: &Address) -> UspsResult {
        if self.consecutive_errors >= 3 {
            return UspsResult::status("skipped_after_service_errors");
        }
        tokio::time::sleep(self.interval).await;
        let result = self.request(address).await;
        if result.service_error() {
            self.consecutive_errors += 1;
        } else {
            self.consecutive_errors = 0;
        }
        result
    }
    async fn request(&self, address: &Address) -> UspsResult {
        let Some(form) = form_fields(address) else {
            return UspsResult::status("invalid_state");
        };
        for attempt in 0..3 {
            let response = self
                .client
                .post(ENDPOINT)
                .header(
                    "Referer",
                    "https://tools.usps.com/zip-code-lookup.htm?byaddress",
                )
                .header("Accept", "application/json")
                .form(&form)
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    if (status.is_server_error() || status.as_u16() == 429) && attempt < 2 {
                        tokio::time::sleep(Duration::from_secs(2 * (attempt + 1))).await;
                        continue;
                    }
                    if !status.is_success() {
                        let mut result = UspsResult::status(format!("http_{}", status.as_u16()));
                        result.raw = serde_json::json!({
                            "http_status": status.as_u16(),
                            "redirect": response.headers().get("location").and_then(|v| v.to_str().ok()),
                        }).to_string();
                        return result;
                    }
                    return match response.json::<Value>().await {
                        Ok(value) => parse_response(value),
                        Err(_) => UspsResult::status("invalid_json"),
                    };
                }
                Err(_) if attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(2 * (attempt + 1))).await
                }
                Err(_) => return UspsResult::status("network_error"),
            }
        }
        UspsResult::status("network_error")
    }
}
fn form_fields(address: &Address) -> Option<[(&str, &str); 8]> {
    Some([
        ("companyName", ""),
        ("address1", &address.line1),
        ("address2", &address.line2),
        ("city", &address.city),
        ("state", state_code(&address.state)?),
        ("urbanCode", ""),
        ("zip", &address.zip),
        ("zip4", ""),
    ])
}
fn field(value: &Value, names: &[&str]) -> String {
    names
        .iter()
        .find_map(|name| {
            value.get(name).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Bool(b) => Some(b.to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
        })
        .unwrap_or_default()
}
fn parse_response(value: Value) -> UspsResult {
    let mut result = UspsResult {
        raw: value.to_string(),
        ..UspsResult::default()
    };
    let status = field(&value, &["resultStatus"]);
    if !status.eq_ignore_ascii_case("SUCCESS") {
        result.status = "lookup_failed".into();
        return result;
    }
    let Some(addresses) = value.get("addressList").and_then(Value::as_array) else {
        result.status = "invalid_response".into();
        return result;
    };
    if addresses.is_empty() {
        result.status = "no_match".into();
        return result;
    }
    if addresses.len() > 1 {
        result.status = "multiple_matches".into();
        return result;
    }
    let address = &addresses[0];
    result.status = "matched".into();
    result.address = [
        field(address, &["addressLine1"]),
        field(address, &["addressLine2"]),
        field(address, &["city"]),
        field(address, &["state"]),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    result.zip5 = field(address, &["zip5"]);
    result.zip4 = field(address, &["zip4"]);
    result.dpv_confirmation = field(address, &["DPVConfirmation", "dpvConfirmation"]);
    result.cmra = field(address, &["DPVCMRA", "dpvCmra"]);
    result.business = field(address, &["business"]);
    result.carrier_route = field(address, &["carrierRoute"]);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_preserves_unit_and_normalizes_state() {
        let address = Address {
            line1: "5953 Mabel Rd".into(),
            line2: "unit-236".into(),
            city: "Las Vegas".into(),
            state: "Nevada".into(),
            zip: "89110".into(),
            zip4: None,
        };
        let form = form_fields(&address).unwrap();
        assert!(form.contains(&("state", "NV")));
        assert!(form.contains(&("address2", "unit-236")));
    }
    #[test]
    fn parses_results_without_inventing_cmra() {
        let result = parse_response(
            serde_json::json!({"resultStatus":"SUCCESS","addressList":[{"addressLine1":"5953 MABEL RD","zip5":"89110","zip4":"1234","DPVConfirmation":"Y","business":"N"}]}),
        );
        assert_eq!(result.status, "matched");
        assert_eq!(result.zip4, "1234");
        assert!(result.cmra.is_empty());
        assert_eq!(
            parse_response(serde_json::json!({"resultStatus":"SUCCESS","addressList":[]})).status,
            "no_match"
        );
        assert_eq!(
            parse_response(serde_json::json!({"resultStatus":"SUCCESS","addressList":[{},{}]}))
                .status,
            "multiple_matches"
        );
        assert!(parse_response(serde_json::json!({"resultStatus":"FAILURE"})).service_error());
        assert!(parse_response(serde_json::json!({"resultStatus":"SUCCESS"})).service_error());
    }
}
