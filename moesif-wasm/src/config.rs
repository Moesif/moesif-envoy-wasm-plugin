use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    #[serde(default = "default_debug")]
    pub debug: bool,
    #[serde(default = "connection_timeout")]
    pub connection_timeout: usize,
}

fn default_batch_max_size() -> usize {
    100
}

fn default_batch_max_wait() -> usize {
    2000
}

fn default_upstream() -> String {
    "moesif_api".to_string()
}

fn default_base_uri() -> String {
    "api.moesif.net".to_string()
}

fn default_debug() -> bool {
    false
}

fn connection_timeout() -> usize {
    5000
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct AppConfigResponse {
    pub org_id: String,
    pub app_id: String,
    pub sample_rate: i32,
    pub block_bot_traffic: bool,
    pub user_sample_rate: HashMap<String, i32>,
    pub company_sample_rate: HashMap<String, i32>,
    pub user_rules: HashMap<String, Vec<EntityRuleValues>>,
    pub company_rules: HashMap<String, Vec<EntityRuleValues>>,
    pub ip_addresses_blocked_by_name: HashMap<String, String>,
    pub regex_config: Vec<RegexRule>,
    pub billing_config_jsons: HashMap<String, String>,
    pub e_tag: Option<String>,
}

impl AppConfigResponse {
    pub fn new() -> AppConfigResponse {
        AppConfigResponse {
            sample_rate: 100,
            ..Default::default()
        }
    }

    pub fn get_sampling_percentage(&self, user_id: Option<&str>, company_id: Option<&str>) -> i32 {
        if let Some(user_id) = user_id {
            if let Some(user_rate) = self.user_sample_rate.get(user_id) {
                return *user_rate;
            }
        }

        if let Some(company_id) = company_id {
            if let Some(company_rate) = self.company_sample_rate.get(company_id) {
                return *company_rate;
            }
        }

        self.sample_rate
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EntityRuleValues {
    pub rules: String,
    pub values: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RegexRule {
    pub conditions: Vec<RegexCondition>,
    pub sample_rate: i32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RegexCondition {
    pub path: String,
    pub value: String,
}
