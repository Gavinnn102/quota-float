use std::{fs, path::PathBuf};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::Value;

use crate::models::{CreditBalance, ProviderSnapshot, UsageWindow};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const RESET_CREDITS_URL: &str = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_AUTH_BYTES: u64 = 256 * 1024;
const FIVE_HOUR_WINDOW_SECONDS: u64 = 18_000;
const WEEKLY_WINDOW_SECONDS: u64 = 604_800;
const WINDOW_DURATION_TOLERANCE_SECONDS: u64 = 60;

struct Auth {
    access_token: String,
    account_id: Option<String>,
}

fn auth_path() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
        .map(|home| home.join("auth.json"))
}

fn pick_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| value.get(*key)?.as_str())
}

fn account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    pick_string(
        &value,
        &[
            "https://api.openai.com/auth.chatgpt_account_id",
            "chatgpt_account_id",
        ],
    )
    .map(str::to_owned)
}

fn load_auth() -> Result<Auth, &'static str> {
    let path = auth_path().ok_or("Codex login was not found.")?;
    let metadata = fs::metadata(&path).map_err(|_| "Please sign in to Codex Desktop first.")?;
    if !metadata.is_file() || metadata.len() > MAX_AUTH_BYTES {
        return Err("Codex login data is unavailable.");
    }
    let raw = fs::read_to_string(path).map_err(|_| "Please sign in to Codex Desktop first.")?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| "Codex login format has changed.")?;
    let tokens = value.get("tokens").unwrap_or(&value);
    let access_token = pick_string(tokens, &["access_token", "accessToken"])
        .ok_or("Codex login expired. Please sign in again.")?
        .to_owned();
    let account_id = pick_string(tokens, &["account_id", "accountId"])
        .map(str::to_owned)
        .or_else(|| account_id_from_jwt(&access_token));
    Ok(Auth {
        access_token,
        account_id,
    })
}

fn headers(auth: &Auth) -> Result<HeaderMap, &'static str> {
    let mut result = HeaderMap::new();
    let mut bearer = HeaderValue::from_str(&format!("Bearer {}", auth.access_token))
        .map_err(|_| "Codex login data is invalid.")?;
    bearer.set_sensitive(true);
    result.insert(AUTHORIZATION, bearer);
    result.insert(ACCEPT, HeaderValue::from_static("application/json"));
    result.insert("originator", HeaderValue::from_static("Codex Desktop"));
    result.insert("OAI-Product-Sku", HeaderValue::from_static("CODEX"));
    if let Some(account_id) = &auth.account_id {
        let mut value =
            HeaderValue::from_str(account_id).map_err(|_| "Account identifier is invalid.")?;
        value.set_sensitive(true);
        result.insert("ChatGPT-Account-Id", value);
    }
    Ok(result)
}

fn number_with_key<'a>(value: &'a Value, keys: &[&'a str]) -> Option<(&'a str, f64)> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_f64().map(|number| (*key, number)))
}

fn integer(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|item| u64::try_from(item).ok()))
    })
}

fn parse_credit_balance(value: Option<&Value>) -> Option<CreditBalance> {
    let value = value?.as_object()?;
    let unlimited = value.get("unlimited").and_then(Value::as_bool) == Some(true);
    let balance = value.get("balance").and_then(|balance| {
        balance
            .as_f64()
            .or_else(|| balance.as_str()?.trim().parse::<f64>().ok())
            .filter(|number| number.is_finite())
    });
    (unlimited || balance.is_some()).then_some(CreditBalance { balance, unlimited })
}

fn timestamp(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let item = value.get(*key)?;
        if let Some(text) = item.as_str() {
            return Some(text.to_owned());
        }
        item.as_i64()
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| time.to_rfc3339())
    })
}

fn collect_reset_credit_expirations(value: &Value) -> Vec<String> {
    fn visit(value: &Value, output: &mut Vec<String>) {
        match value {
            Value::Array(items) => {
                for item in items {
                    visit(item, output);
                }
            }
            Value::Object(map) => {
                if let Some(time) = timestamp(
                    value,
                    &[
                        "expires_at",
                        "expiresAt",
                        "expiration_time",
                        "expirationTime",
                        "expires",
                    ],
                ) {
                    output.push(time);
                }
                for key in [
                    "credits",
                    "reset_credits",
                    "resetCredits",
                    "available",
                    "items",
                    "grants",
                ] {
                    if let Some(child) = map.get(key) {
                        visit(child, output);
                    }
                }
            }
            _ => {}
        }
    }

    let mut expirations = Vec::new();
    visit(value, &mut expirations);
    expirations.sort();
    expirations.dedup();
    expirations
}

fn scale_ratio_field(key: &str, value: f64) -> bool {
    matches!(
        key,
        "remaining_ratio" | "remainingRatio" | "used_ratio" | "usedRatio" | "utilization"
    ) || (!key.contains("percent") && !key.contains("pct") && value <= 1.0)
}

fn parse_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?;
    let remaining_percent = if let Some((key, remaining)) = number_with_key(
        value,
        &[
            "remaining_percent",
            "remainingPercent",
            "remaining_pct",
            "remainingPct",
            "remaining_ratio",
            "remainingRatio",
            "remaining",
        ],
    ) {
        if scale_ratio_field(key, remaining) {
            remaining * 100.0
        } else {
            remaining
        }
    } else {
        let (key, used) = number_with_key(
            value,
            &[
                "used_percent",
                "usedPercent",
                "used_pct",
                "usedPct",
                "used_ratio",
                "usedRatio",
                "utilization",
                "used",
            ],
        )?;
        let used_percent = if scale_ratio_field(key, used) {
            used * 100.0
        } else {
            used
        };
        100.0 - used_percent
    };
    Some(UsageWindow {
        remaining_percent: remaining_percent.clamp(0.0, 100.0),
        resets_at: timestamp(
            value,
            &[
                "reset_at",
                "resetAt",
                "resets_at",
                "resetsAt",
                "reset_time",
                "resetTime",
            ],
        ),
        window_seconds: integer(
            value,
            &[
                "limit_window_seconds",
                "limitWindowSeconds",
                "window_seconds",
                "windowSeconds",
                "duration_seconds",
                "durationSeconds",
                "period_seconds",
                "periodSeconds",
            ],
        )
        .unwrap_or(0),
    })
}

fn normalized_window_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowNameMatch {
    None,
    Generic,
    Specific,
}

fn matches_window_name(value: &str, names: &[&str]) -> WindowNameMatch {
    let value = normalized_window_name(value);
    if value.is_empty()
        || !names.iter().any(|name| {
            let name = normalized_window_name(name);
            !name.is_empty() && value.contains(&name)
        })
    {
        return WindowNameMatch::None;
    }

    if [
        "week",
        "weekly",
        "weekwindow",
        "weeklywindow",
        "fivehour",
        "fivehours",
        "fivehourwindow",
    ]
    .iter()
    .any(|name| value.contains(name))
    {
        WindowNameMatch::Specific
    } else {
        WindowNameMatch::Generic
    }
}

fn matches_window_duration(window: &UsageWindow, expected_seconds: u64) -> bool {
    expected_seconds > 0
        && window.window_seconds > 0
        && window.window_seconds.abs_diff(expected_seconds) <= WINDOW_DURATION_TOLERANCE_SECONDS
}

fn push_window_candidate<'a>(
    candidates: &mut Vec<WindowCandidate<'a>>,
    value: &'a Value,
    name: Option<&str>,
    names: &[&str],
) {
    if parse_window(Some(value)).is_some() {
        let name_match = name
            .map(|name| matches_window_name(name, names))
            .unwrap_or(WindowNameMatch::None);
        candidates.push(WindowCandidate { value, name_match });
    }
}

struct WindowCandidate<'a> {
    value: &'a Value,
    name_match: WindowNameMatch,
}

fn collect_window_collection<'a>(
    collection: &'a Value,
    names: &[&str],
    candidates: &mut Vec<WindowCandidate<'a>>,
) {
    match collection {
        Value::Array(items) => {
            for item in items {
                let item_name = pick_string(item, &["name", "type", "id", "window", "label"]);
                push_window_candidate(candidates, item, item_name, names);
            }
        }
        Value::Object(items) => {
            for (key, item) in items {
                push_window_candidate(candidates, item, Some(key), names);
            }
        }
        _ => {}
    }
}

fn is_window_collection_key(key: &str) -> bool {
    matches!(
        normalized_window_name(key).as_str(),
        "windows" | "limitwindows" | "limits" | "buckets"
    )
}

fn find_window<'a>(
    rate_limit: &'a Value,
    names: &[&str],
    expected_seconds: u64,
) -> Option<&'a Value> {
    let mut candidates = Vec::new();
    if parse_window(Some(rate_limit)).is_some() {
        candidates.push(WindowCandidate {
            value: rate_limit,
            name_match: WindowNameMatch::None,
        });
    }

    if let Some(object) = rate_limit.as_object() {
        for (key, value) in object {
            if is_window_collection_key(key) {
                collect_window_collection(value, names, &mut candidates);
            } else {
                push_window_candidate(&mut candidates, value, Some(key), names);
            }
        }
    } else {
        collect_window_collection(rate_limit, names, &mut candidates);
    }

    // The service has used both primary_window and secondary_window for the
    // weekly allowance. Prefer an actual duration over a label so a server-side
    // rename or swap cannot make the two quota windows appear in the wrong slot.
    if expected_seconds > 0 {
        if let Some(candidate) = candidates.iter().find(|candidate| {
            parse_window(Some(candidate.value))
                .is_some_and(|window| matches_window_duration(&window, expected_seconds))
        }) {
            return Some(candidate.value);
        }

        let has_explicit_duration = candidates.iter().any(|candidate| {
            parse_window(Some(candidate.value)).is_some_and(|window| window.window_seconds > 0)
        });
        if has_explicit_duration {
            // A descriptive period name remains unambiguous when an older or
            // mixed response omits its duration. Generic primary/secondary
            // labels remain ambiguous once another candidate has a duration.
            if let Some(candidate) = candidates.iter().find(|candidate| {
                candidate.name_match == WindowNameMatch::Specific
                    && parse_window(Some(candidate.value))
                        .is_some_and(|window| window.window_seconds == 0)
            }) {
                return Some(candidate.value);
            }
            return None;
        }
    }

    candidates.into_iter().find_map(|candidate| {
        (candidate.name_match != WindowNameMatch::None).then_some(candidate.value)
    })
}

fn parse_period_window(
    rate_limit: &Value,
    names: &[&str],
    expected_seconds: u64,
) -> Option<UsageWindow> {
    let mut window = parse_window(find_window(rate_limit, names, expected_seconds))?;
    if window.window_seconds == 0 {
        // A named legacy window has no duration in the payload, but its slot
        // still identifies the period. Preserve that information for callers.
        window.window_seconds = expected_seconds;
    }
    Some(window)
}

fn parse_quota_windows(rate_limit: &Value) -> (Option<UsageWindow>, Option<UsageWindow>) {
    let five_hour_window = parse_period_window(
        rate_limit,
        &[
            "primary_window",
            "primaryWindow",
            "five_hour_window",
            "fiveHourWindow",
            "five_hours",
            "fiveHours",
            "five_hour",
            "fiveHour",
            "primary",
        ],
        FIVE_HOUR_WINDOW_SECONDS,
    );
    let weekly_window = parse_period_window(
        rate_limit,
        &[
            "secondary_window",
            "secondaryWindow",
            "weekly_window",
            "weeklyWindow",
            "week_window",
            "weekWindow",
            "weekly",
            "secondary",
        ],
        WEEKLY_WINDOW_SECONDS,
    );
    (weekly_window, five_hour_window)
}

fn safe_http_failure(status: reqwest::StatusCode) -> (&'static str, &'static str) {
    match status.as_u16() {
        401 | 403 => ("signed_out", "Codex login expired. Please sign in again."),
        429 => (
            "unavailable",
            "Quota service is rate limited. It will retry automatically.",
        ),
        _ => ("unavailable", "Quota service is temporarily unavailable."),
    }
}

async fn limited_json(mut response: reqwest::Response) -> Result<Value, ()> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if bytes.len().saturating_add(chunk.len()) as u64 > MAX_RESPONSE_BYTES {
            return Err(());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| ())
}

pub async fn fetch_snapshot(client: &reqwest::Client) -> ProviderSnapshot {
    let auth = match load_auth() {
        Ok(value) => value,
        Err(message) => return ProviderSnapshot::failure("signed_out", message),
    };
    let request_headers = match headers(&auth) {
        Ok(value) => value,
        Err(message) => return ProviderSnapshot::failure("signed_out", message),
    };

    let (usage_result, credits_result) = tokio::join!(
        client
            .get(USAGE_URL)
            .headers(request_headers.clone())
            .send(),
        client
            .get(RESET_CREDITS_URL)
            .headers(request_headers)
            .send(),
    );

    let usage_response = match usage_result {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            let (status, message) = safe_http_failure(response.status());
            return ProviderSnapshot::failure(status, message);
        }
        Err(_) => {
            return ProviderSnapshot::failure(
                "unavailable",
                "Network unavailable. It will retry automatically.",
            )
        }
    };
    let usage: Value = match limited_json(usage_response).await {
        Ok(value) => value,
        Err(_) => {
            return ProviderSnapshot::failure("unavailable", "Quota response format has changed.")
        }
    };
    let rate_limit = usage
        .get("rate_limit")
        .or_else(|| usage.get("rateLimit"))
        .unwrap_or(&usage);
    let (weekly_window, five_hour_window) = parse_quota_windows(rate_limit);
    if weekly_window.is_none() && five_hour_window.is_none() {
        return ProviderSnapshot::failure(
            "unavailable",
            "Quota response is missing a quota window.",
        );
    }

    let usage_credits = usage
        .get("rate_limit_reset_credits")
        .or_else(|| usage.get("rateLimitResetCredits"));
    let usage_reset_credits = usage_credits.and_then(|value| {
        integer(
            value,
            &[
                "available_count",
                "availableCount",
                "remaining",
                "count",
                "quantity",
            ],
        )
    });
    let usage_reset_credit_expires_at = usage_credits
        .map(collect_reset_credit_expirations)
        .unwrap_or_default();

    let (reset_credits, reset_credit_expires_at) = match credits_result {
        Ok(response) if response.status().is_success() => match limited_json(response).await.ok() {
            Some(value) => (
                integer(
                    &value,
                    &[
                        "available_count",
                        "availableCount",
                        "remaining",
                        "count",
                        "quantity",
                    ],
                )
                .or(usage_reset_credits),
                {
                    let expirations = collect_reset_credit_expirations(&value);
                    if expirations.is_empty() {
                        usage_reset_credit_expires_at
                    } else {
                        expirations
                    }
                },
            ),
            None => (usage_reset_credits, usage_reset_credit_expires_at),
        },
        _ => (usage_reset_credits, usage_reset_credit_expires_at),
    };

    ProviderSnapshot {
        provider: "codex".into(),
        display_name: "CODEX".into(),
        plan: pick_string(&usage, &["plan_type", "planType"]).map(|value| value.to_uppercase()),
        weekly_window,
        five_hour_window,
        credits: parse_credit_balance(usage.get("credits")),
        reset_credits,
        reset_credit_expires_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
        status: "ok".into(),
        message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credit_balances_without_confusing_them_with_reset_credits() {
        for balance in [
            serde_json::json!("1211.0000000000"),
            serde_json::json!(1211),
        ] {
            let usage = serde_json::json!({
                "credits": {"has_credits": true, "unlimited": false, "balance": balance},
                "rate_limit_reset_credits": {"available_count": 3}
            });
            let credits = parse_credit_balance(usage.get("credits")).unwrap();
            assert_eq!(credits.balance, Some(1211.0));
            assert!(!credits.unlimited);
            assert!(parse_credit_balance(usage.get("rate_limit_reset_credits")).is_none());
        }
    }

    #[test]
    fn preserves_zero_and_negative_credit_balances() {
        for balance in ["0", "-2.5"] {
            let value = serde_json::json!({"has_credits": false, "balance": balance});
            assert_eq!(
                parse_credit_balance(Some(&value)).unwrap().balance,
                balance.parse::<f64>().ok()
            );
        }
    }

    #[test]
    fn missing_or_invalid_credit_balances_remain_unknown() {
        assert!(parse_credit_balance(None).is_none());
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"has_credits": false}),
            serde_json::json!({"balance": null}),
            serde_json::json!({"balance": ""}),
            serde_json::json!({"balance": "unknown"}),
            serde_json::json!({"balance": "NaN"}),
            serde_json::json!({"balance": "Infinity"}),
            serde_json::json!({"balance": "1e999"}),
            serde_json::json!({"balance": true}),
        ] {
            assert!(parse_credit_balance(Some(&value)).is_none(), "{value}");
        }
    }

    #[test]
    fn unlimited_credits_do_not_require_a_numeric_balance() {
        let value = serde_json::json!({"unlimited": true, "balance": null});
        let credits = parse_credit_balance(Some(&value)).unwrap();
        assert!(credits.unlimited);
        assert!(credits.balance.is_none());
    }

    #[test]
    fn parses_both_window_shapes() {
        let snake = serde_json::json!({
            "used_percent": 26,
            "reset_at": 1738300000,
            "limit_window_seconds": 18000
        });
        let window = parse_window(Some(&snake)).unwrap();
        assert_eq!(window.remaining_percent, 74.0);
        assert_eq!(window.window_seconds, 18000);
        let camel = serde_json::json!({
            "utilization": 0.4,
            "resetsAt": "2026-07-07T00:00:00Z",
            "windowSeconds": 604800
        });
        assert_eq!(parse_window(Some(&camel)).unwrap().remaining_percent, 60.0);
    }

    #[test]
    fn prefers_explicit_remaining_percent() {
        let value = serde_json::json!({
            "remainingPercent": 73.4,
            "usedPercent": 99,
            "resetTime": "2026-07-07T00:00:00Z",
            "durationSeconds": 18000
        });
        let window = parse_window(Some(&value)).unwrap();
        assert_eq!(window.remaining_percent, 73.4);
        assert_eq!(window.window_seconds, 18000);
    }

    #[test]
    fn treats_fractional_percent_fields_as_ratios() {
        let explicit_remaining = serde_json::json!({"remaining": 0.25, "periodSeconds": 18000});
        assert_eq!(
            parse_window(Some(&explicit_remaining))
                .unwrap()
                .remaining_percent,
            25.0
        );

        let used_ratio = serde_json::json!({"used": 0.25, "periodSeconds": 18000});
        assert_eq!(
            parse_window(Some(&used_ratio)).unwrap().remaining_percent,
            75.0
        );
    }

    #[test]
    fn does_not_scale_explicit_percent_fields() {
        let explicit_remaining =
            serde_json::json!({"remaining_percent": 0.4, "windowSeconds": 18000});
        assert_eq!(
            parse_window(Some(&explicit_remaining))
                .unwrap()
                .remaining_percent,
            0.4
        );

        let explicit_used = serde_json::json!({"used_percent": 0.4, "windowSeconds": 18000});
        assert_eq!(
            parse_window(Some(&explicit_used))
                .unwrap()
                .remaining_percent,
            99.6
        );
    }

    #[test]
    fn finds_window_by_duration_or_name_in_arrays() {
        let rate_limit = serde_json::json!({
            "windows": [
                {"name": "weekly", "remainingPercent": 88, "windowSeconds": 604800},
                {"name": "primary", "remainingPercent": 51, "windowSeconds": 18000}
            ]
        });
        let short = parse_window(find_window(
            &rate_limit,
            &["primary_window", "primary"],
            18_000,
        ))
        .unwrap();
        let weekly = parse_window(find_window(
            &rate_limit,
            &["secondary_window", "weekly"],
            604_800,
        ))
        .unwrap();
        assert_eq!(short.remaining_percent, 51.0);
        assert_eq!(weekly.remaining_percent, 88.0);
    }

    #[test]
    fn identifies_windows_by_duration_when_primary_and_secondary_are_swapped() {
        let rate_limit = serde_json::json!({
            "primary_window": {
                "used_percent": 11,
                "limit_window_seconds": 604800
            },
            "secondary_window": {
                "used_percent": 22,
                "limit_window_seconds": 18000
            }
        });

        let (weekly, five_hour) = parse_quota_windows(&rate_limit);
        let weekly = weekly.expect("weekly duration should win over the field name");
        let five_hour = five_hour.expect("five-hour duration should win over the field name");
        assert_eq!(weekly.remaining_percent, 89.0);
        assert_eq!(weekly.window_seconds, WEEKLY_WINDOW_SECONDS);
        assert_eq!(five_hour.remaining_percent, 78.0);
        assert_eq!(five_hour.window_seconds, FIVE_HOUR_WINDOW_SECONDS);
    }

    #[test]
    fn does_not_use_a_named_window_with_the_wrong_duration() {
        let rate_limit = serde_json::json!({
            "primary_window": {
                "remaining_percent": 51,
                "window_seconds": 604800
            }
        });

        assert!(find_window(
            &rate_limit,
            &["primary_window", "primary"],
            FIVE_HOUR_WINDOW_SECONDS,
        )
        .is_none());
    }

    #[test]
    fn keeps_a_snapshot_usable_when_only_one_window_is_present() {
        let rate_limit = serde_json::json!({
            "primary_window": {
                "remainingPercent": 63,
                "windowSeconds": 18000
            }
        });

        let (weekly, five_hour) = parse_quota_windows(&rate_limit);
        assert!(weekly.is_none());
        assert_eq!(five_hour.unwrap().remaining_percent, 63.0);
    }

    #[test]
    fn uses_a_descriptive_name_for_a_missing_duration_in_a_mixed_response() {
        let rate_limit = serde_json::json!({
            "primary_window": {
                "remainingPercent": 70,
                "windowSeconds": 18000
            },
            "weekly_window": {
                "remainingPercent": 40
            }
        });

        let (weekly, five_hour) = parse_quota_windows(&rate_limit);
        let weekly = weekly.expect("weekly_window should remain available");
        let five_hour = five_hour.expect("primary five-hour window should remain available");
        assert_eq!(weekly.remaining_percent, 40.0);
        assert_eq!(weekly.window_seconds, WEEKLY_WINDOW_SECONDS);
        assert_eq!(five_hour.remaining_percent, 70.0);
        assert_eq!(five_hour.window_seconds, FIVE_HOUR_WINDOW_SECONDS);
    }

    #[test]
    fn finds_weekly_window_by_duration_when_service_uses_primary() {
        let rate_limit = serde_json::json!({
            "allowed": true,
            "primary_window": {
                "used_percent": 24,
                "limit_window_seconds": 604800,
                "reset_after_seconds": 596723,
                "reset_at": 1784521572
            },
            "secondary_window": null
        });
        let weekly = parse_window(find_window(
            &rate_limit,
            &["secondary_window", "weekly"],
            604_800,
        ))
        .unwrap();
        assert_eq!(weekly.remaining_percent, 76.0);
        assert_eq!(weekly.window_seconds, 604_800);
    }
}
