use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Headers = Vec<(String, String)>;
type Body = Vec<u8>;

// Handler is the type of the callback function that will be called when the HTTP call response is received.
pub type Handler = Box<dyn Fn(Headers, Option<Body>) + Send>;

#[derive(Default)]
pub struct HttpCallbackManager {
    // token_id -> callback response handler function
    handlers: Arc<Mutex<HashMap<u32, Handler>>>,
}

impl HttpCallbackManager {
    pub fn register_handler(&self, token_id: u32, handler: Handler) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.insert(token_id, handler);
    }

    pub fn handle_response(&self, token_id: u32, headers: Headers, body: Option<Body>) {
        let mut handlers = self.handlers.lock().unwrap();

        if let Some(handler) = handlers.remove(&token_id) {
            handler(headers, body);
        } else {
            // This should never happen, but if it does, 
            // it represents a bug in Envoy which we can only log
            log::error!("Envoy called on_http_call_response with non-existant token: {}", token_id);
        }
    }
}

pub fn get_header(headers: &Headers, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, header_value)| header_value.to_owned())
}