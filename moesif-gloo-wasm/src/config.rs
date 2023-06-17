use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Default, Clone)]
pub struct Config {
    pub env: EnvConfig,
    pub event_queue_id: u32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    pub moesif_application_id: String,
    pub user_id_header: Option<String>,
    pub company_id_header: Option<String>,
    #[serde(default = "default_batch_max_size")]
    pub batch_max_size: usize,
    #[serde(default = "default_batch_max_wait")]
    pub batch_max_wait: usize,
    #[serde(default = "default_upstream")]
    pub upstream: String,
    #[serde(default = "default_base_uri")]
    pub base_uri: String,
}

fn default_batch_max_size() -> usize {
    10
}

fn default_batch_max_wait() -> usize {
    2
}

fn default_upstream() -> String {
    "moesif_api".to_string()
}

fn default_base_uri() -> String {
    "api.moesif.net".to_string()
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct AppConfigResponse {
    org_id: String,
    app_id: String,
    sample_rate: i32,
    block_bot_traffic: bool,
    user_sample_rate: HashMap<String, i32>,
    company_sample_rate: HashMap<String, i32>,
    user_rules: HashMap<String, Vec<EntityRuleValues>>,
    company_rules: HashMap<String, Vec<EntityRuleValues>>,
    ip_addresses_blocked_by_name: HashMap<String, String>,
    regex_config: Vec<RegexRule>,
    billing_config_jsons: HashMap<String, String>,
    e_tag: String,
}

impl AppConfigResponse {
    pub fn new() -> AppConfigResponse {
        AppConfigResponse {
            sample_rate: 100,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EntityRuleValues {
    rules: String,
    values: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RegexRule {
    conditions: Vec<RegexCondition>,
    sample_rate: i32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RegexCondition {
    path: String,
    value: String,
}

pub async fn get_app_config() -> Result<AppConfigResponse, Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let url = format!("{}/v1/config", CONFIG.base_uri);
    let response = client.get(&url)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("X-Moesif-Application-Id", &CONFIG.moesif_application_id)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let mut config: AppConfigResponse = serde_json::from_str(&response.text().await?)?;
    config.e_tag = response.headers().get("X-Moesif-Config-Etag").unwrap().to_str().unwrap().to_string();
    Ok(config)
}

pub fn get_sampling_percentage(user_id: Option<&str>, company_id: Option<&str>) -> i32 {
    let config = APP_CONFIG.read().unwrap();

    if let Some(user_id) = user_id {
        if let Some(user_rate) = config.user_sample_rate.get(user_id) {
            return *user_rate;
        }
    }

    if let Some(company_id) = company_id {
        if let Some(company_rate) = config.company_sample_rate.get(company_id) {
            return *company_rate;
        }
    }

    config.sample_rate
}
