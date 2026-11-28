//! The console and network feed shown by the developer tools overlay.
//! Capacity-capped so a chatty page can never balloon memory. The raw
//! payloads are the same JSON strings the injected instrumentation has
//! always emitted; parsing them here is page instrumentation plumbing,
//! not the UI channel, which stays binary. (Port of the old devtools log
//! store.)

use crate::wire::{ConsoleItem, DevToolsSnapshot, NetworkItem};

pub const DEFAULT_CAPACITY: usize = 500;

pub struct DevToolsLogStore {
    console: Vec<ConsoleItem>,
    network: Vec<NetworkItem>,
    capacity: usize,
}

impl DevToolsLogStore {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            console: Vec::new(),
            network: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Ingests one raw instrumentation payload. Malformed payloads are
    /// dropped, matching the old decoder.
    pub fn handle_raw(&mut self, raw: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return;
        };
        let object = match value.as_object() {
            Some(object) => object,
            None => return,
        };
        let at_ms = object
            .get("at")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        match object.get("kind").and_then(|v| v.as_str()) {
            Some("console") => {
                let level = object
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("log")
                    .to_string();
                let message = object
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                self.console.push(ConsoleItem { level, message, at_ms });
                self.trim();
            }
            Some("network") => {
                let method = object
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET")
                    .to_string();
                let url = object
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let status = object.get("status").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let duration_ms = object
                    .get("durationMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                self.network.push(NetworkItem {
                    method,
                    url,
                    status,
                    duration_ms,
                    at_ms,
                });
                self.trim();
            }
            _ => {}
        }
    }

    pub fn append_console(&mut self, level: &str, message: &str) {
        self.console.push(ConsoleItem {
            level: level.to_string(),
            message: message.to_string(),
            at_ms: 0,
        });
        self.trim();
    }

    pub fn clear(&mut self) {
        self.console.clear();
        self.network.clear();
    }

    pub fn console_len(&self) -> usize {
        self.console.len()
    }

    pub fn network_len(&self) -> usize {
        self.network.len()
    }

    pub fn snapshot(&self) -> DevToolsSnapshot {
        DevToolsSnapshot {
            console: self.console.clone(),
            network: self.network.clone(),
        }
    }

    fn trim(&mut self) {
        while self.console.len() > self.capacity {
            self.console.remove(0);
        }
        while self.network.len() > self.capacity {
            self.network.remove(0);
        }
    }
}

impl Default for DevToolsLogStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_and_network_payloads_are_ingested() {
        let mut store = DevToolsLogStore::new();
        store.handle_raw(
            r#"{"kind":"console","level":"error","message":"boom","at":1700000000000}"#,
        );
        store.handle_raw(
            r#"{"kind":"network","method":"POST","url":"https://x.example/api","status":204,"durationMs":12,"at":1700000000001}"#,
        );
        store.handle_raw("not json at all");
        store.handle_raw(r#"{"kind":"unknown"}"#);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.console.len(), 1);
        assert_eq!(snapshot.console[0].level, "error");
        assert_eq!(snapshot.console[0].at_ms, 1_700_000_000_000);
        assert_eq!(snapshot.network.len(), 1);
        assert_eq!(snapshot.network[0].method, "POST");
        assert_eq!(snapshot.network[0].status, 204);
        assert_eq!(snapshot.network[0].duration_ms, 12);
    }

    #[test]
    fn defaults_fill_missing_fields() {
        let mut store = DevToolsLogStore::new();
        store.handle_raw(r#"{"kind":"console"}"#);
        store.handle_raw(r#"{"kind":"network"}"#);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.console[0].level, "log");
        assert_eq!(snapshot.console[0].message, "");
        assert_eq!(snapshot.network[0].method, "GET");
        assert_eq!(snapshot.network[0].status, 0);
    }

    #[test]
    fn the_feed_stays_capped() {
        let mut store = DevToolsLogStore::with_capacity(3);
        for index in 0..50 {
            store.append_console("log", &format!("line {index}"));
        }
        assert_eq!(store.console_len(), 3);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.console[0].message, "line 47");
        assert_eq!(snapshot.console[2].message, "line 49");
        store.clear();
        assert_eq!(store.console_len(), 0);
        assert_eq!(store.network_len(), 0);
    }

    #[test]
    fn load_errors_land_in_the_console() {
        let mut store = DevToolsLogStore::new();
        store.append_console("error", "Failed to load: net::ERR_CACHE_MISS");
        assert_eq!(store.snapshot().console[0].message, "Failed to load: net::ERR_CACHE_MISS");
    }
}
