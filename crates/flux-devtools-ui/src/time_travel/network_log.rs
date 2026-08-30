//! Network inspector log (spec §5.3, FLUX-060): the retained HTTP traffic
//! stream shown by the network inspector.
//!
//! A bounded, FIFO buffer of [`NetworkRecord`] entries. It is decoupled from
//! gpui and from any live host socket so it is unit-testable without a device.
//! The host emits `TelemetryEvent::NetworkRequest` / `NetworkResponse` (the wire
//! variant this record is reconstructed from); the DevTools pairs a response
//! with its request by `request_id` so the inspector can show a single row with
//! method, URL, status, and latency.

/// The lifecycle phase a network record is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkPhase {
    /// A request went out but no response has arrived yet.
    Pending,
    /// The request resolved (Ready or Error); see [`NetworkRecord::result_kind`].
    Complete,
}

impl NetworkPhase {
    /// The stable single-character tag for the inspector.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Pending => "…",
            Self::Complete => "✓",
        }
    }
}

/// One retained HTTP exchange (request, plus the response once it arrives).
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkRecord {
    /// Stable id pairing the request with its response.
    pub request_id: u32,
    /// HTTP method (GET/POST/…).
    pub method: String,
    /// Fully-qualified request URL.
    pub url: String,
    /// Request body snippet (truncated), if any.
    pub request_body: Option<String>,
    /// Capability id that issued the request (diagnostics).
    pub capability_id: u32,
    /// Lifecycle phase: `Pending` until a response lands.
    pub phase: NetworkPhase,
    /// Response status code, once known.
    pub status_code: Option<u16>,
    /// Response latency in ms, once known.
    pub latency_ms: Option<u32>,
    /// Response body snippet (truncated), if any.
    pub response_body: Option<String>,
    /// `0`=Pending, `1`=Ready, `2`=Error (cell state, ADR-0044).
    pub result_kind: Option<u8>,
}

impl NetworkRecord {
    /// Builds a record from a request event (always starts `Pending`).
    #[must_use]
    pub fn from_request(
        request_id: u32,
        method: impl Into<String>,
        url: impl Into<String>,
        request_body: Option<String>,
        capability_id: u32,
    ) -> Self {
        Self {
            request_id,
            method: method.into(),
            url: url.into(),
            request_body,
            capability_id,
            phase: NetworkPhase::Pending,
            status_code: None,
            latency_ms: None,
            response_body: None,
            result_kind: None,
        }
    }

    /// Attaches a response, moving the record to `Complete`.
    pub fn attach_response(
        &mut self,
        status_code: u16,
        latency_ms: u32,
        response_body: Option<String>,
        result_kind: u8,
    ) {
        self.phase = NetworkPhase::Complete;
        self.status_code = Some(status_code);
        self.latency_ms = Some(latency_ms);
        self.response_body = response_body;
        self.result_kind = Some(result_kind);
    }

    /// Whether the request errored (cell state `2`).
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.result_kind == Some(2)
    }

    /// A compact one-line summary for the inspector: `M GET url → 200 (42ms)`.
    #[must_use]
    pub fn render(&self) -> String {
        let status = match self.status_code {
            Some(code) => format!(" → {code}"),
            None => String::new(),
        };
        let latency = match self.latency_ms {
            Some(ms) => format!(" ({ms}ms)"),
            None => String::new(),
        };
        format!(
            "{} {} {}{}{}",
            self.phase.tag(),
            self.method,
            self.url,
            status,
            latency
        )
    }
}

/// A bounded FIFO of [`NetworkRecord`] exchanges (newest at the end).
///
/// Capacity is fixed; when full, the oldest record is evicted (oldest-first),
/// matching the timeline buffer's eviction contract. A request whose response
/// arrives after eviction simply never pairs — acceptable, since the inspector
/// is a recent-traffic window, not an audit log.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkLog {
    records: Vec<NetworkRecord>,
    capacity: usize,
}

impl NetworkLog {
    /// Creates an empty log holding at most `capacity` records.
    ///
    /// A zero capacity is rejected (an inspector with no room is a logic error);
    /// callers must supply a positive capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "NetworkLog capacity must be positive");
        Self {
            records: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Records an outbound request, starting the exchange `Pending`. If a record
    /// with the same `request_id` already exists (a protocol anomaly), it is
    /// replaced so the newest request wins.
    pub fn push_request(&mut self, record: NetworkRecord) {
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|r| r.request_id == record.request_id)
        {
            *existing = record;
        } else {
            if self.records.len() == self.capacity {
                self.records.remove(0);
            }
            self.records.push(record);
        }
    }

    /// Attaches a response to the pending request with `request_id`. If no such
    /// request is retained (evicted or never seen), the response is dropped
    /// silently — the inspector only shows complete exchanges it can attribute.
    pub fn push_response(
        &mut self,
        request_id: u32,
        status_code: u16,
        latency_ms: u32,
        response_body: Option<String>,
        result_kind: u8,
    ) {
        if let Some(record) = self.records.iter_mut().find(|r| r.request_id == request_id) {
            record.attach_response(status_code, latency_ms, response_body, result_kind);
        }
    }

    /// Number of retained records (pending + complete).
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log currently holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// A snapshot of the retained records in insertion order (oldest first).
    #[must_use]
    pub fn snapshot(&self) -> Vec<NetworkRecord> {
        self.records.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> NetworkRecord {
        NetworkRecord::from_request(1, "GET", "https://api.example.com/users", None, 14)
    }

    #[test]
    fn pending_request_renders_without_status() {
        let r = request();
        assert_eq!(r.phase, NetworkPhase::Pending);
        assert_eq!(r.render(), "… GET https://api.example.com/users");
    }

    #[test]
    fn response_completes_the_exchange() {
        let mut log = NetworkLog::new(8);
        log.push_request(request());
        log.push_response(1, 200, 42, Some("{\"ok\":true}".into()), 1);

        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        let r = &snap[0];
        assert_eq!(r.phase, NetworkPhase::Complete);
        assert_eq!(r.status_code, Some(200));
        assert_eq!(r.latency_ms, Some(42));
        assert!(!r.is_error());
        assert_eq!(
            r.render(),
            "✓ GET https://api.example.com/users → 200 (42ms)"
        );
    }

    #[test]
    fn error_response_is_flagged() {
        let mut log = NetworkLog::new(8);
        log.push_request(request());
        log.push_response(1, 500, 7, Some("boom".into()), 2);
        assert!(log.snapshot()[0].is_error());
    }

    #[test]
    fn response_without_request_is_dropped() {
        // A response whose request was never retained must not fabricate a row.
        let mut log = NetworkLog::new(8);
        log.push_response(99, 200, 1, None, 1);
        assert!(log.is_empty());
    }

    #[test]
    fn capacity_evicts_oldest_request() {
        let mut log = NetworkLog::new(2);
        log.push_request(NetworkRecord::from_request(1, "GET", "u1", None, 14));
        log.push_request(NetworkRecord::from_request(2, "GET", "u2", None, 14));
        log.push_request(NetworkRecord::from_request(3, "GET", "u3", None, 14));
        let snap = log.snapshot();
        assert_eq!(snap.len(), 2);
        // request id 1 (oldest) was evicted; 2 and 3 remain.
        assert!(snap.iter().all(|r| r.request_id != 1));
        assert!(snap.iter().any(|r| r.request_id == 2));
        assert!(snap.iter().any(|r| r.request_id == 3));
    }

    #[test]
    fn duplicate_request_id_replaces() {
        let mut log = NetworkLog::new(8);
        log.push_request(NetworkRecord::from_request(1, "GET", "old", None, 14));
        log.push_request(NetworkRecord::from_request(1, "POST", "new", None, 14));
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].method, "POST");
        assert_eq!(snap[0].url, "new");
    }
}
