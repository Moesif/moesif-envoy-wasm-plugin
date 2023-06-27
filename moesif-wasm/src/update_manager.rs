use chrono::{DateTime, Utc};

#[derive(Default)]
pub struct UpdateManager<T> {
    current_etag: String,
    old_etag: String,
    last_updated: i64,
    data: T,
}

impl<T> UpdateManager<T> {
    fn update(&mut self, etag: String, data: T) {
        self.old_etag = self.current_etag.clone();
        self.current_etag = etag;
        self.last_updated = Utc::now().timestamp();
        self.data = data;
    }

    fn is_updated(&self, etag: &str) -> bool {
        self.current_etag != etag && self.old_etag != etag
    }

    fn is_expired(&self, ttl: i64) -> bool {
        self.last_updated + ttl < Utc::now().timestamp()
    }

    fn get_data(&self) -> &T {
        &self.data
    }
}