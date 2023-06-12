use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::Engine as _;
use chrono::Utc;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use serde::Deserialize;
use serde::Serialize;

const EVENT_QUEUE: &str = "moesif_event_queue";

#[derive(Default, Clone, Serialize, Deserialize)]
struct Config {
    moesif_application_id: String,
    user_id_header: Option<String>,
    company_id_header: Option<String>,
    #[serde(default)]
    queue_id: u32,
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
    status: usize,
    headers: HashMap<String, String>,
    ip_address: Option<String>,
    body: serde_json::Value,
}

#[derive(Default, Serialize, Deserialize)]
struct Event {
    request: RequestInfo,
    response: Option<ResponseInfo>,
    user_id: Option<String>,
    company_id: Option<String>,
    metadata: serde_json::Value,
}

#[derive(Default)]
pub struct EventHttpContext {
    config: Arc<Config>,
    event: Event,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
}

impl Context for EventHttpContext {}

impl HttpContext for EventHttpContext {
    fn on_http_request_headers(&mut self, _: usize, _: bool) -> Action {
        self.event.request.time = Utc::now().to_rfc3339();
        self.event.request.headers = EventHttpContext::header_list_to_map(self.get_http_request_headers());

        self.event.request.uri = self.get_http_request_header(":path").unwrap_or_default();
        self.event.request.verb = self.get_http_request_header(":method").unwrap_or_else(|| "GET".into());
        self.event.request.api_version = self.get_http_request_header("x-api-version");
        self.event.request.ip_address = self.get_http_request_header("x-forwarded-for");
        self.event.request.transfer_encoding = self.get_http_request_header("transfer-encoding");
        self.event.request.headers.retain(|k, _| !k.starts_with(":"));
        if let Some(user_id_header) = &self.config.user_id_header {
            self.event.user_id = self.get_http_request_header(user_id_header)
        }
        if let Some(company_id_header) = &self.config.company_id_header {
            self.event.company_id = self.get_http_request_header(company_id_header);
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
            let content_type = self.event.request.headers.get("content-type");
            self.event.request.body = EventHttpContext::body_bytes_to_value(body, content_type);
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _: usize, _: bool) -> Action {
        let status_str = self.get_http_response_header(":status").unwrap_or("0".to_string());
        let mut response = ResponseInfo {
            time: Utc::now().to_rfc3339(),
            status: status_str.parse::<usize>().unwrap_or(0),
            headers: EventHttpContext::header_list_to_map(self.get_http_response_headers()),
            ip_address: self.get_http_response_header("x-forwarded-for"),
            body: serde_json::Value::Null,
        };
        response.headers.retain(|k, _| !k.starts_with(":"));
        self.event.response = Some(response);
        Action::Continue
    }

    fn on_http_response_body(&mut self, num_elements: usize, end_of_stream: bool) -> Action {
        if let Some(body_bytes) = self.get_http_response_body(0, num_elements) {
            self.response_body.extend(body_bytes);
        }

        if end_of_stream {
            if let Some(response) = self.event.response.as_mut() {
                // response_body moved by mem::take which is used to avoid copying it unnecessarily
                let body = std::mem::take(&mut self.response_body);
                let content_type = response.headers.get("content-type");
                response.body = EventHttpContext::body_bytes_to_value(body, content_type);
            }
        }

        Action::Continue
    }

    fn on_log(&mut self) {
        let json = serde_json::to_string(&self.event).unwrap();
        log::info!("Request & Response Data: {}", json);
        self.enqueue_event();
    }
}

impl EventHttpContext {
    fn enqueue_event(self: &EventHttpContext) {
        let event_bytes = serde_json::to_vec(&self.event).unwrap();

        match self.enqueue_shared_queue(self.config.queue_id, Some(&event_bytes)) {
            Ok(_) => {
                log::info!("Enqueued event to shared queue");
            }
            Err(e) => {
                log::error!("Failed to enqueue event: {:?}", e);
            }
        }
    }


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
                };
            }
        }

        let body_str = String::from_utf8_lossy(&body).into_owned();
        serde_json::Value::String(body_str)
    }
}


#[derive(Default)]
pub struct EventRootContext {
    config: Arc<Config>,
    event_byte_buffer: Arc<Mutex<Vec<Bytes>>>,
}

impl Context for EventRootContext {
    fn on_http_call_response(
        &mut self,
        token_id: u32,
        num_headers: usize,
        body_size: usize,
        num_trailers: usize,
    ) {
        log::info!(
            "EventRootContext HTTP callback -> Token ID: {}, Number of Headers: {}, Body Size: {}, Number of Trailers: {}",
            token_id,
            num_headers,
            body_size,
            num_trailers
        );

        // To access the headers, body, and trailers
        let headers = self.get_http_call_response_headers();
        let body = self.get_http_call_response_body(0, body_size);
        let trailers = self.get_http_call_response_trailers();

        // Log headers
        for (name, value) in &headers {
            log::info!("Header: {} - {}", name, value);
        }

        // Log body
        if let Some(body_bytes) = body {
            let body_str = std::str::from_utf8(&body_bytes).unwrap_or_default();
            log::info!("Body: {}", body_str);
        }

        // Log trailers
        for (name, value) in &trailers {
            log::info!("Trailer: {} - {}", name, value);
        }
    }
}

impl RootContext for EventRootContext {
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
                Ok(mut config) => {
                    config.queue_id = self.register_shared_queue(EVENT_QUEUE);
                    self.config = Arc::new(config);
                    log::info!("Loaded configuration: {:?}", self.config.moesif_application_id);
                    true
                }
                Err(e) => {
                    // This will also catch the error when moesif_application_id is missing
                    log::error!("Failed to parse configuration: {:?}", e);
                    false
                }
            }
        } else {
            log::error!("Failed to read configuration");
            false
        }
    }

    fn on_tick(&mut self) {
        log::debug!("on_tick: {}", Utc::now().to_rfc3339());
        self.poll_queue();
    }

    fn on_queue_ready(&mut self, _queue_id: u32) {
        log::debug!("on_queue_ready: {}", _queue_id);
        self.poll_queue();
    }

    fn create_http_context(&self, _: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(EventHttpContext {
            config: Arc::clone(&self.config),
            ..Default::default()
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

impl EventRootContext {
    fn poll_queue(&self) {
        match self.dequeue_shared_queue(self.config.queue_id) {
            Ok(Some(event_bytes)) => {
                self.add_event(event_bytes);
            }
            Ok(None) => {}
            Err(e) => {
                log::error!("Failed to dequeue event: {:?}", e);
            }
        }
    }
    fn add_event(&self, event_bytes: Bytes) {
        let mut buffer: MutexGuard<Vec<Bytes>> = self.event_byte_buffer.lock().unwrap();
        buffer.push(event_bytes);
        if buffer.len() >= 10 {
            // buffer is a Vec<Bytes>, so we need to write it as a JSON array [event1, event2, ...]
            let body = self.write_events_json(buffer.drain(..).collect());
            self.dispatch_http_event(body);
        }
    }

    // write vector of already serialized events as a JSON array
    fn write_events_json(&self, events: Vec<Bytes>) -> Bytes {
        // Calculate the total size of all event bytes
        let total_size: usize = events.iter().map(|event_bytes| event_bytes.len()).sum();
        // total_size + commas + brackets
        let json_array_size = total_size + events.len() - 1 + 2;
        let mut event_json_array = Vec::with_capacity(json_array_size);

        // Write the already serialized event JSON into a JSON array [event1, event2, ...]
        event_json_array.push(b'[');
        for (i, event_bytes) in events.iter().enumerate() {
            if i > 0 {
                event_json_array.push(b',');
            }
            event_json_array.extend(event_bytes);
        }
        event_json_array.push(b']');

        event_json_array
    }

    fn dispatch_http_event(&self, body: Bytes) -> u32 {
        // TODO: make these optional configs.
        // The upstream name and authority for the collector events endpoint
        let upstream = "moesif_api";
        let authority = "api-dev.moesif.net";
        let content_length = body.len().to_string();
        let application_id = self.config.moesif_application_id.clone();
        let headers = vec![
            (":scheme", "https"),
            (":method", "POST"),
            (":path", "/v1/events/batch"),
            (":authority", authority),
            ("accept", "*/*"),
            ("content-type", "application/json"),
            ("content-length", &content_length),
            ("x-moesif-application-id", &application_id),
        ];
        let trailers = vec![];
        let timeout = Duration::from_secs(5);

        // Dispatch the HTTP request. The result is a token that uniquely identifies this call
        match self.dispatch_http_call(upstream, headers, Some(&body), trailers, timeout) {
            Ok(token_id) => {
                log::debug!("Dispatched HTTP call with token ID {}", token_id);
                token_id
            }
            Err(e) => {
                log::error!("Failed to dispatch HTTP call: {:?}", e);
                0
            }
        }
    }
}

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Debug);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> { Box::new(EventRootContext::default()) });
}}
