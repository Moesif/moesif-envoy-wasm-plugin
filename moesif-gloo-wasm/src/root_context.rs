use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::Utc;
use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::{Bytes, ContextType};

use crate::config::{Config, EnvConfig};
use crate::http_context::EventHttpContext;

const EVENT_QUEUE: &str = "moesif_event_queue";

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
            match serde_json::from_str::<EnvConfig>(config_str) {
                Ok(env) => {
                    let config = Config {
                        env,
                        event_queue_id: self.register_shared_queue(EVENT_QUEUE),
                    };
                    self.config = Arc::new(config);
                    log::info!(
                        "Loaded configuration: {:?}",
                        self.config.env.moesif_application_id
                    );
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
        // This will send all events in the buffer to enforce the batch_max_wait
        self.drain_and_send(1);
    }

    fn on_queue_ready(&mut self, _queue_id: u32) {
        log::debug!("on_queue_ready: {}", _queue_id);
        self.poll_queue();
        // This will send all full batches in the buffer to enforce the batch_max_size
        self.drain_and_send(self.config.env.batch_max_size);
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
    // dequeue all events and add them to the buffer until the queue is empty
    fn poll_queue(&self) {
        let mut more = true;
        while more {
            match self.dequeue_shared_queue(self.config.event_queue_id) {
                Ok(Some(event_bytes)) => {
                    self.add_event(event_bytes);
                }
                Ok(None) => {
                    more = false;
                }
                Err(e) => {
                    more = false;
                    log::error!("Failed to dequeue event: {:?}", e);
                }
            }
        }
    }

    fn add_event(&self, event_bytes: Bytes) {
        let mut buffer: MutexGuard<Vec<Bytes>> = self.event_byte_buffer.lock().unwrap();
        buffer.push(event_bytes);
    }

    fn drain_and_send(&self, drain_at_least: usize) {
        let mut buffer: MutexGuard<Vec<Bytes>> = self.event_byte_buffer.lock().unwrap();
        while buffer.len() >= drain_at_least {
            let end = std::cmp::min(buffer.len(), self.config.env.batch_max_size);
            let body = self.write_events_json(buffer.drain(..end).collect());
            self.dispatch_http_request("POST", "/v1/events/batch", body);
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

    fn dispatch_http_request(&self, method: &str, path: &str,  body: Bytes) -> u32 {
        let content_length = body.len().to_string();
        let application_id = self.config.env.moesif_application_id.clone();
        let headers = vec![
            (":scheme", "https"),
            (":method", method),
            (":path", path),
            (":authority", &self.config.env.base_uri),
            ("accept", "*/*"),
            ("content-type", "application/json"),
            ("content-length", &content_length),
            ("x-moesif-application-id", &application_id),
        ];
        let trailers = vec![];
        let timeout = Duration::from_secs(5);

        // Dispatch the HTTP request. The result is a token that uniquely identifies this call
        match self.dispatch_http_call(&self.config.env.upstream, headers, Some(&body), trailers, timeout) {
            Ok(token_id) => {
                log::info!("Dispatched request to {} and got token {}", path, token_id);
                token_id
            }
            Err(e) => {
                log::error!("Failed to dispatch HTTP call: {:?}", e);
                0
            }
        }
    }
}
