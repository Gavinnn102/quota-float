use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub remaining_percent: f64,
    pub resets_at: Option<String>,
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditBalance {
    /// Service credit units, before conversion to the displayed USD amount.
    pub balance: Option<f64>,
    pub unlimited: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub provider: String,
    pub display_name: String,
    pub plan: Option<String>,
    pub weekly_window: Option<UsageWindow>,
    pub five_hour_window: Option<UsageWindow>,
    pub credits: Option<CreditBalance>,
    pub reset_credits: Option<u64>,
    pub reset_credit_expires_at: Vec<String>,
    pub updated_at: String,
    pub status: String,
    pub message: Option<String>,
}

impl ProviderSnapshot {
    pub fn failure(status: &str, message: &str) -> Self {
        Self {
            provider: "codex".into(),
            display_name: "CODEX".into(),
            plan: None,
            weekly_window: None,
            five_hour_window: None,
            credits: None,
            reset_credits: None,
            reset_credit_expires_at: Vec::new(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            status: status.into(),
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPreferences {
    #[serde(default = "default_panel_visible")]
    pub panel_visible: bool,
    #[serde(default = "default_expanded")]
    pub expanded: bool,
    #[serde(default = "default_always_on_top")]
    pub always_on_top: bool,
    pub pinned_provider: Option<String>,
    pub auto_rotate_seconds: u64,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_quota_window")]
    pub quota_window: String,
}

fn default_always_on_top() -> bool {
    true
}
fn default_panel_visible() -> bool {
    true
}
fn default_expanded() -> bool {
    true
}
fn default_language() -> String {
    "zh-CN".into()
}
fn default_quota_window() -> String {
    "weekly".into()
}

impl Default for WidgetPreferences {
    fn default() -> Self {
        Self {
            panel_visible: true,
            expanded: true,
            always_on_top: true,
            pinned_provider: None,
            auto_rotate_seconds: 12,
            language: default_language(),
            quota_window: default_quota_window(),
        }
    }
}

impl WidgetPreferences {
    pub fn normalized(mut self) -> Self {
        self.auto_rotate_seconds = self.auto_rotate_seconds.clamp(5, 300);
        if self.pinned_provider.as_deref() != Some("codex") {
            self.pinned_provider = None;
        }
        if self.language != "en" && self.language != "zh-CN" {
            self.language = default_language();
        }
        if self.quota_window != "weekly" && self.quota_window != "fiveHour" {
            self.quota_window = default_quota_window();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetPreferences;

    #[test]
    fn older_preferences_ignore_and_remove_the_retired_lock_setting() {
        let preferences = serde_json::from_str::<WidgetPreferences>(
            r#"{"locked":true,"alwaysOnTop":true,"pinnedProvider":null,"autoRotateSeconds":12,"language":"zh-CN"}"#,
        )
        .expect("older settings should remain readable");

        assert!(preferences.panel_visible);
        assert!(preferences.expanded);
        assert!(preferences.always_on_top);
        assert_eq!(preferences.quota_window, "weekly");
        let serialized = serde_json::to_value(preferences).expect("settings should serialize");
        assert!(serialized.get("locked").is_none());
        assert_eq!(
            serialized
                .get("quotaWindow")
                .and_then(|value| value.as_str()),
            Some("weekly")
        );
    }

    #[test]
    fn invalid_quota_window_falls_back_to_weekly() {
        let preferences = serde_json::from_str::<WidgetPreferences>(
            r#"{"quotaWindow":"daily","alwaysOnTop":true,"pinnedProvider":null,"autoRotateSeconds":12,"language":"zh-CN"}"#,
        )
        .expect("settings with a future quota window should remain readable")
        .normalized();

        assert_eq!(preferences.quota_window, "weekly");
    }

    #[test]
    fn five_hour_quota_window_survives_normalization() {
        let preferences = serde_json::from_str::<WidgetPreferences>(
            r#"{"quotaWindow":"fiveHour","alwaysOnTop":true,"pinnedProvider":null,"autoRotateSeconds":12,"language":"zh-CN"}"#,
        )
        .expect("five-hour quota selection should be readable")
        .normalized();

        assert_eq!(preferences.quota_window, "fiveHour");
    }
}
