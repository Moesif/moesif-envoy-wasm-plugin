mod config;
mod event;
mod http_context;
mod root_context;
mod http_callback;
mod rules;
mod update_manager;

use proxy_wasm::{traits::RootContext, types::LogLevel};
use root_context::EventRootContext;

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Debug);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> { 
        Box::new(EventRootContext::default()) 
    });
}}
