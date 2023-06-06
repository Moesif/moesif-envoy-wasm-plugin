use chrono::Utc;
use log::info;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use serde::{Deserialize, Serialize};
use std::str;

#[derive(Serialize, Deserialize)]
struct RequestInfo {
    time: String,
    headers: Vec<(String, String)>,
    uri: String,
    body: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct ResponseInfo {
    status: String,
    time: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct HttpRequestData {
    request: RequestInfo,
    response: Option<ResponseInfo>,
}

pub struct HttpLogger {
    data: HttpRequestData,
}

impl HttpContext for HttpLogger {
        fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
            self.data.request = RequestInfo {
            time: Utc::now().to_rfc3339(),
            headers: self.get_http_request_headers(),
            uri: self.get_http_request_header(":path").unwrap_or_default(),
            body: Vec::new(),
        };

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
}

impl Context for HttpLogger {
    fn on_done(&mut self) -> bool {
        let json = serde_json::to_string(&self.data).unwrap();
        info!("Request & Response Data: {}", json);

        true
    }
}
