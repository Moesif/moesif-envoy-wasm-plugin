use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{DateTime, Utc};
use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::{Bytes, ContextType};

use crate::config::{AppConfigResponse, Config, EnvConfig};
use crate::http_callback::{get_header, Handler, HttpCallbackManager};
use crate::http_context::EventHttpContext;
use crate::rules::{GovernanceRule, GovernanceRulesResponse, template, Variable};

const EVENT_QUEUE: &str = "moesif_event_queue";

#[derive(Default)]
pub struct EventRootContext {
    context_id: String,
    config: Arc<Config>,
    is_start: bool,
    event_byte_buffer: Arc<Mutex<Vec<Bytes>>>,
    http_manager: HttpCallbackManager,
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

        self.http_manager.handle_response(token_id, headers, body);
    }
}

impl RootContext for EventRootContext {
    fn on_vm_start(&mut self, _: usize) -> bool {
        self.context_id = uuid::Uuid::new_v4().to_string();
        self.set_tick_period(Duration::from_millis(1));
        self.is_start = true;
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
        log::debug!("on_tick context_id {} at {}", self.context_id, Utc::now().to_rfc3339());
        // We set on_tick to 1ms at start up to work around a bug or limitation in Envoy
        // where dispatch http call does not work in on_configure or on_vm_start.
        // We set it back to 2s after the first tick.
        if self.is_start {
            self.is_start = false;
            let foo = self.set_tick_period(Duration::from_secs(2));
            self.request_config_api();
            self.request_rules_api();
        }
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
            self.dispatch_http_request(
                "POST",
                "/v1/events/batch",
                body,
                Box::new(|headers, _| {
                    let config_etag = get_header(&headers, "X-Moesif-Config-Etag");
                    let rules_etag = get_header(&headers, "X-Moesif-Rules-Etag");
                    log::info!("Event Response eTags: config={:?} rules={:?}", config_etag, rules_etag);
                }),
            );
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

    fn request_config_api(&self) {
        self.dispatch_http_request(
            "GET",
            "/v1/config",
            Bytes::new(),
            Box::new(|headers, body| {
                log::info!("Config Response headers: {:?}", headers);
                if let Some(body) = body {
                    let mut app_config_response =
                        serde_json::from_slice::<AppConfigResponse>(&body).unwrap();
                    app_config_response.e_tag = get_header(&headers, "X-Moesif-Config-Etag");
                    log::info!(
                        "Config Response app_config_response: {:?}",
                        app_config_response
                    );
                } else {
                    log::warn!("Config Response body: None");
                }
            }),
        );
    }

    fn request_rules_api(&self) {
        self.dispatch_http_request(
            "GET",
            "/v1/rules",
            Bytes::new(),
            Box::new(|headers, body| {
                log::info!("Rules Response headers: {:?}", headers);
                let e_tag = get_header(&headers, "X-Moesif-Config-Etag");
                if let Some(body) = body {
                    // what to do in these callbacks on error?
                    let rules = serde_json::from_slice::<Vec<GovernanceRule>>(&body).unwrap();
                    let rules_response = GovernanceRulesResponse { rules, e_tag };
                    log::info!("Rules Response rules_response: {:?}", rules_response);
                    for rule in rules_response.rules {
                        if let (Some(body), Some(variables)) = (rule.response.body, rule.variables)
                        {
                            log::info!("Rule body: {:?}", body);
                            log::info!("Rule variables: {:?}", variables);
                            let variables: HashMap<String, String> = variables
                                .into_iter()
                                .map(|variable| (variable.name, variable.path))
                                .collect();
                            let templated_body = template(&body.0, &variables);
                            log::info!("Rule templated_body: {:?}", templated_body);
                        }
                    }
                } else {
                    log::warn!("Rules Response body: None");
                }
            }),
        );
    }

    fn dispatch_http_request(
        &self,
        method: &str,
        path: &str,
        body: Bytes,
        callback: Handler,
    ) -> u32 {
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
        // encode body as a string to print
        let bodystr = std::str::from_utf8(&body).unwrap_or_default();
        log::info!(
            "Dispatching {} upstream {} request to {} with body {}",
            &self.config.env.upstream,
            method,
            path,
            bodystr
        );
        // log headers
        for (name, value) in &headers {
            log::info!("Header: {}: {}", name, value);
        }
        // Dispatch the HTTP request. The result is a token that uniquely identifies this call
        match self.dispatch_http_call(
            &self.config.env.upstream,
            headers,
            Some(&body),
            trailers,
            timeout,
        ) {
            Ok(token_id) => {
                log::info!("Dispatched request to {} and got token {}", path, token_id);
                self.http_manager.register_handler(token_id, callback);
                token_id
            }
            Err(e) => {
                log::error!("Failed to dispatch HTTP call: {:?}", e);
                0
            }
        }
    }
}
