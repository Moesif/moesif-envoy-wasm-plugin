use chrono::Utc;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

#[derive(Default, Serialize, Deserialize)]
struct Config {
    moesif_application_id: Option<String>,
    user_id_header: Option<String>,
    company_id_header: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct RequestInfo {
    time: String,
    headers: Vec<(String, String)>,
    uri: String,
    body: Vec<u8>,
}

#[derive(Default, Serialize, Deserialize)]
struct ResponseInfo {
    status: String,
    time: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Default, Serialize, Deserialize)]
struct HttpRequestData {
    request: RequestInfo,
    response: Option<ResponseInfo>,
}

#[derive(Default)]
pub struct HttpLogger {
    config: Config,
    data: HttpRequestData,
}

impl Context for HttpLogger {}

impl HttpContext for HttpLogger {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        self.data.request.time = Utc::now().to_rfc3339();
        self.data.request.headers = self.get_http_request_headers();
        self.data.request.uri = self.get_http_request_header(":path").unwrap_or_default();

        Action::Continue
    }

    fn on_http_request_body(&mut self, _num_elements: usize, _end_of_stream: bool) -> Action {
        if let Some(body_bytes) = self.get_http_request_body(0, _num_elements) {
            self.data.request.body.extend(body_bytes);
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        self.data.response = Some(ResponseInfo {
            status: self.get_http_response_header(":status").unwrap_or_default(),
            time: Utc::now().to_rfc3339(),
            headers: self.get_http_response_headers(),
            body: Vec::new(),
        });

        Action::Continue
    }

    fn on_http_response_body(&mut self, _num_elements: usize, _end_of_stream: bool) -> Action {
        if let Some(body_bytes) = self.get_http_response_body(0, _num_elements) {
            if let Some(ref mut response) = self.data.response {
                response.body.extend(body_bytes);
            }
        }

        Action::Continue
    }

    fn on_log(&mut self) {
        let json = serde_json::to_string(&self.data).unwrap();
        log::info!("Request & Response Data: {}", json);
    }
}

impl RootContext for HttpLogger {
    fn create_http_context(&self, _: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(HttpLogger::default()))
    }

    fn on_vm_start(&mut self, _: usize) -> bool {
        let tick_period = Duration::from_secs(20);
        self.set_tick_period(tick_period);
        let config = self.get_vm_configuration();
        log::info!("VM configuration: {:?}", config);
        true
    }

    fn on_configure(&mut self, _: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            let config_str = std::str::from_utf8(&config_bytes).unwrap();
            match serde_json::from_str::<Config>(config_str) {
                Ok(config) => {
                    if config.moesif_application_id.is_none() {
                        log::error!("Missing required moesif_application_id in configuration.");
                        return false;
                    }
                    self.config = config;
                    log::info!("Loaded configuration: {:?}", self.config.moesif_application_id);
                    return true;
                }
                Err(e) => {
                    log::error!("Failed to parse configuration: {:?}", e);
                    return false;
                }
            }
        }
        log::error!("Failed to read configuration.");
        false
    }

    fn on_tick(&mut self) {
        let now = Utc::now();
        log::info!("on_tick: {}", now.to_rfc3339());
    }

    fn on_queue_ready(&mut self, _queue_id: u32) {
        log::info!("on_queue_ready: {}", _queue_id);
    }
}

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> { Box::new(HttpLogger::default()) });
}}
