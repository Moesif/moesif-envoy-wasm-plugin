use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use chrono::Utc;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Clone, Serialize, Deserialize)]
struct Config {
    moesif_application_id: String,
    user_id_header: Option<String>,
    company_id_header: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct RequestInfo {
    time: String,
    verb: String,
    uri: String,
    headers: HashMap<String, String>,
    transfer_encoding: Option<String>,
    api_version: Option<String>,
    ip_address: Option<String>,
    body: serde_json::Value,
}

#[derive(Default, Serialize, Deserialize)]
struct ResponseInfo {
    time: String,
    status: String,
    headers: HashMap<String, String>,
    ip_address: Option<String>,
    body: serde_json::Value,
}

#[derive(Default, Serialize, Deserialize)]
struct HttpRequestData {
    request: RequestInfo,
    response: Option<ResponseInfo>,
    user_id: Option<String>,
    company_id: Option<String>,
    metadata: serde_json::Value,
}

#[derive(Default)]
pub struct HttpLogger {
    config: Arc<Config>,
    data: HttpRequestData,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
}

impl Context for HttpLogger {}

impl HttpContext for HttpLogger {
    fn on_http_request_headers(&mut self, _: usize, _: bool) -> Action {
        self.data.request.time = Utc::now().to_rfc3339();
        self.data.request.headers = header_list_to_map(self.get_http_request_headers());

        self.data.request.uri = self.get_http_request_header(":path").unwrap_or_default();
        self.data.request.verb = self.get_http_request_header(":method").unwrap_or_else(|| "GET".into());
        self.data.request.api_version = self.get_http_request_header("x-api-version");
        self.data.request.ip_address = self.get_http_request_header("x-forwarded-for");
        self.data.request.transfer_encoding = self.get_http_request_header("transfer-encoding");
        self.data.request.headers.retain(|k, _| !k.starts_with(":"));
        if let Some(user_id_header) = &self.config.user_id_header {
            self.data.user_id = self.get_http_request_header(user_id_header)
        }
        if let Some(company_id_header) = &self.config.company_id_header {
            self.data.company_id = self.get_http_request_header(company_id_header);
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, _num_elements: usize, end_of_stream: bool) -> Action {
        if let Some(body_bytes) = self.get_http_request_body(0, _num_elements) {
            self.request_body.extend(body_bytes);
        }

        if end_of_stream {
            // request_body is not readable after mem::take which is used to avoid copying it unnecessarily
            let body = std::mem::take(&mut self.request_body);
            let content_type = self.data.request.headers.get("content-type");
            self.data.request.body = body_bytes_to_value(body, content_type);
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _: usize, _: bool) -> Action {
        let mut response = ResponseInfo {
            time: Utc::now().to_rfc3339(),
            status: self.get_http_response_header(":status").unwrap_or_default(),
            headers: header_list_to_map(self.get_http_response_headers()),
            ip_address: self.get_http_request_header("x-forwarded-for"),
            body: serde_json::Value::Null,
        };
        response.headers.retain(|k, _| !k.starts_with(":"));
        self.data.response = Some(response);
        Action::Continue
    }

    fn on_http_response_body(&mut self, num_elements: usize, end_of_stream: bool) -> Action {
        if let Some(body_bytes) = self.get_http_response_body(0, num_elements) {
            self.response_body.extend(body_bytes);
        }

        if end_of_stream {
            if let Some(response) = self.data.response.as_mut() {
                // response_body is not readable after mem::take which is used to avoid copying it unnecessarily
                let body = std::mem::take(&mut self.response_body);
                let content_type = response.headers.get("content-type");
                response.body = body_bytes_to_value(body, content_type);
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
                    self.config = Arc::new(config);
                    log::info!("Loaded configuration: {:?}", self.config.moesif_application_id);
                    true
                },
                Err(e) => {
                    // This will also catch the error when moesif_application_id is missing
                    log::error!("Failed to parse configuration: {:?}", e);
                    false
                }
            }
        } else {
            log::error!("Failed to read configuration.");
            false
        }
    }

    fn on_tick(&mut self) {
        let now = Utc::now();
        log::debug!("on_tick: {}", now.to_rfc3339());
    }

    fn on_queue_ready(&mut self, _queue_id: u32) {
        log::info!("on_queue_ready: {}", _queue_id);
    }

    fn create_http_context(&self, _: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(HttpLogger{
            config: Arc::clone(&self.config),
            ..Default::default()
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> { Box::new(HttpLogger::default()) });
}}

fn header_list_to_map(headers: Vec<(String, String)>) -> HashMap<String, String> {
    headers.into_iter().collect::<HashMap<_, _>>()
}

fn body_bytes_to_value(body: Vec<u8>, content_type: Option<&String>) -> serde_json::Value {
    if body.is_empty() {
        return serde_json::Value::Null;
    }

    if let Some(content_type) = content_type {
        if content_type.as_str() == "application/json" {
            return match serde_json::from_slice::<serde_json::Value>(&body) {
                Ok(json) => json,
                Err(_) => {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&body);
                    serde_json::Value::String(encoded)
                }
            }
        }
    }

    let body_str = String::from_utf8_lossy(&body).into_owned();
    serde_json::Value::String(body_str)
}
