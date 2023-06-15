use serde::{Deserialize, Serialize};

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
}

fn default_batch_max_size() -> usize {
    10
}

fn default_batch_max_wait() -> usize {
    2
}