use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use base64::Engine as _;
use chrono::Utc;
use proxy_wasm::traits::{Context, HttpContext};
use proxy_wasm::types::Action;

use crate::config::Config;
use crate::event::{Event, ResponseInfo};

#[derive(Default)]
pub(crate) struct EventHttpContext {
    pub(crate) config: Arc<Config>,
    pub(crate) event: Event,
    pub(crate) request_body: Vec<u8>,
    pub(crate) response_body: Vec<u8>,
}

impl Context for EventHttpContext {}

impl HttpContext for EventHttpContext {
    fn on_http_request_headers(&mut self, _: usize, _: bool) -> Action {
        self.event.request.time = Utc::now().to_rfc3339();
        self.event.request.headers =
            EventHttpContext::header_list_to_map(self.get_http_request_headers());
        // read values from the special :path and :method headers and any other : prefixed headers before removing them
        self.event.request.uri = self.event.request.headers.get(":path").unwrap_or(&"".into()).clone();
        self.event.request.verb = self
            .get_http_request_header(":method")
            .unwrap_or_else(|| "GET".into());
        // remove the special : prefixed headers
        self.event
            .request
            .headers
            .retain(|k, _| !k.starts_with(":"));

        self.event.request.ip_address = EventHttpContext::get_client_ip(&self.event.request.headers);
        self.event.request.api_version = self.get_http_request_header("x-api-version");
        self.event.request.transfer_encoding = self.get_http_request_header("transfer-encoding");
        
        if let Some(user_id_header) = &self.config.env.user_id_header {
            self.event.user_id = self.get_http_request_header(user_id_header)
        }
        if let Some(company_id_header) = &self.config.env.company_id_header {
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
        let status_str = self
            .get_http_response_header(":status")
            .unwrap_or("0".to_string());
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

        match self.enqueue_shared_queue(self.config.event_queue_id, Some(&event_bytes)) {
            Ok(_) => {
                log::info!("Enqueued event to shared queue");
            }
            Err(e) => {
                log::error!("Failed to enqueue event: {:?}", e);
            }
        }
    }

    fn header_list_to_map(headers: Vec<(String, String)>) -> HashMap<String, String> {
        headers
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect::<HashMap<_, _>>()
    }

    fn get_client_ip(headers: &HashMap<String, String>) -> Option<String> {
        let possible_headers = vec![
            "x-client-ip",
            "x-forwarded-for",
            "cf-connecting-ip",
            "fastly-client-ip",
            "true-client-ip",
            "x-real-ip",
            "x-cluster-client-ip",
            "x-forwarded",
            "forwarded-for",
            "forwarded",
            "x-appengine-user-ip",
            "cf-pseudo-ipv4",
        ];

        for header in possible_headers {
            if let Some(value) = headers.get(header) {
                let ips: Vec<&str> = value.split(",").collect();
                for ip in ips {
                    if IpAddr::from_str(ip.trim()).is_ok() {
                        return Some(ip.trim().to_string());
                    }
                }
            }
        }
        None
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
