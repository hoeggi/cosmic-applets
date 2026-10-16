// SPDX-License-Identifier: MPL-2.0

//! Minimal read-only client for the Transmission RPC API.
//!
//! Speaks the JSON-RPC 2.0 dialect with snake_case names that Transmission 4.1
//! introduced (the older protocol is deprecated). Spec:
//! <https://github.com/transmission/transmission/blob/main/docs/rpc-spec.md>

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

const SESSION_HEADER: &str = "X-Transmission-Session-Id";
const TIMEOUT: Duration = Duration::from_secs(10);

const TORRENT_FIELDS: &[&str] = &[
    "id",
    "name",
    "status",
    "percent_done",
    "rate_download",
    "rate_upload",
    "eta",
    "upload_ratio",
    "error",
    "error_string",
    "size_when_done",
    "left_until_done",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// Connection failed or timed out.
    Unreachable(String),
    /// Wrong or missing username / password (HTTP 401).
    Unauthorized,
    /// Any other unexpected HTTP status.
    Http(u16),
    /// The server answered with a JSON-RPC error.
    Server(String),
    /// The answer is not JSON-RPC 2.0, e.g. a server older than 4.1.
    Protocol(String),
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(err) => write!(f, "unreachable: {err}"),
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::Http(code) => write!(f, "HTTP {code}"),
            Self::Server(msg) => write!(f, "server error: {msg}"),
            Self::Protocol(msg) => write!(f, "protocol error: {msg}"),
        }
    }
}

/// One torrent as returned by `torrent_get` (only the fields we request).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Torrent {
    pub id: i64,
    pub name: String,
    /// 0 stopped, 1 queued to verify, 2 verifying, 3 queued to download,
    /// 4 downloading, 5 queued to seed, 6 seeding.
    pub status: i64,
    pub percent_done: f64,
    /// Bytes per second.
    pub rate_download: i64,
    pub rate_upload: i64,
    /// Seconds; -1 not available, -2 unknown.
    pub eta: i64,
    /// -1 not available, -2 infinite.
    pub upload_ratio: f64,
    /// 0 ok, 1 tracker warning, 2 tracker error, 3 local error.
    pub error: i64,
    pub error_string: String,
    pub size_when_done: i64,
    pub left_until_done: i64,
}

/// Everything the applet shows, fetched in one poll.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub torrents: Vec<Torrent>,
    /// Alternative ("turtle") speed limits are on.
    pub alt_speed: bool,
}

#[derive(Debug, Deserialize)]
struct TorrentGet {
    torrents: Vec<Torrent>,
}

#[derive(Debug, Deserialize)]
struct SessionGet {
    #[serde(default)]
    alt_speed_enabled: bool,
}

pub struct Client {
    http: reqwest::Client,
    url: String,
    auth: Option<(String, String)>,
    /// CSRF token handed out by the server with a 409; reused until it expires.
    session_id: Mutex<Option<String>>,
}

impl Client {
    /// Basic auth is used when `username` is not empty.
    pub fn new(url: String, username: &str, password: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            http,
            url,
            auth: (!username.is_empty())
                .then(|| (username.to_owned(), password.unwrap_or_default())),
            session_id: Mutex::new(None),
        }
    }

    pub async fn snapshot(&self) -> Result<Snapshot, RpcError> {
        let torrents: TorrentGet = self
            .call("torrent_get", json!({ "fields": TORRENT_FIELDS }))
            .await?;
        let session: SessionGet = self
            .call("session_get", json!({ "fields": ["alt_speed_enabled"] }))
            .await?;
        Ok(Snapshot {
            torrents: torrents.torrents,
            alt_speed: session.alt_speed_enabled,
        })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, RpcError> {
        let body = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 });
        // The first request (or one with an expired token) gets a 409 carrying the
        // current session ID; resend once with it.
        for _ in 0..2 {
            let mut request = self.http.post(&self.url).json(&body);
            if let Some((user, password)) = &self.auth {
                request = request.basic_auth(user, Some(password));
            }
            if let Some(id) = self.session_id() {
                request = request.header(SESSION_HEADER, id);
            }
            let response = request
                .send()
                .await
                .map_err(|err| RpcError::Unreachable(err.to_string()))?;
            match response.status().as_u16() {
                200 => {
                    let text = response
                        .text()
                        .await
                        .map_err(|err| RpcError::Unreachable(err.to_string()))?;
                    return parse_response(&text);
                }
                401 => return Err(RpcError::Unauthorized),
                409 => {
                    let id = response
                        .headers()
                        .get(SESSION_HEADER)
                        .and_then(|v| v.to_str().ok())
                        .ok_or_else(|| RpcError::Protocol("409 without session ID".into()))?;
                    self.set_session_id(id.to_owned());
                }
                code => return Err(RpcError::Http(code)),
            }
        }
        Err(RpcError::Protocol("session ID rejected".into()))
    }

    fn session_id(&self) -> Option<String> {
        self.session_id.lock().ok().and_then(|id| id.clone())
    }

    fn set_session_id(&self, id: String) {
        if let Ok(mut current) = self.session_id.lock() {
            *current = Some(id);
        }
    }
}

/// Extracts `result` from a JSON-RPC 2.0 response body.
fn parse_response<T: DeserializeOwned>(body: &str) -> Result<T, RpcError> {
    let mut value: Value =
        serde_json::from_str(body).map_err(|err| RpcError::Protocol(err.to_string()))?;
    if let Some(error) = value.get("error") {
        let msg = error
            .pointer("/data/error_string")
            .or_else(|| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(RpcError::Server(msg.to_owned()));
    }
    if value.get("jsonrpc").is_none() {
        // Pre-4.1 servers answer {"result": "method name not recognized", ...}.
        return Err(RpcError::Protocol(
            "not a JSON-RPC 2.0 response (Transmission 4.1 or newer is required)".into(),
        ));
    }
    let result = value
        .get_mut("result")
        .map(Value::take)
        .ok_or_else(|| RpcError::Protocol("response without result".into()))?;
    serde_json::from_value(result).map_err(|err| RpcError::Protocol(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_torrent_get() {
        let body = include_str!("../tests/fixtures/torrent_get.json");
        let parsed: TorrentGet = parse_response(body).unwrap();
        assert_eq!(parsed.torrents.len(), 6);
        let ubuntu = &parsed.torrents[0];
        assert_eq!(ubuntu.name, "ubuntu-26.04-desktop-amd64.iso");
        assert_eq!(ubuntu.status, 4);
        assert!((ubuntu.percent_done - 0.64).abs() < 1e-9);
        assert_eq!(parsed.torrents[5].error, 2);
    }

    #[test]
    fn parses_session_get() {
        let body = r#"{"jsonrpc":"2.0","result":{"alt_speed_enabled":true},"id":1}"#;
        let parsed: SessionGet = parse_response(body).unwrap();
        assert!(parsed.alt_speed_enabled);
    }

    #[test]
    fn reports_jsonrpc_errors() {
        let body = r#"{"jsonrpc":"2.0","error":{"code":3,"message":"Invalid params","data":{"error_string":"no such field"}},"id":1}"#;
        let err = parse_response::<SessionGet>(body).unwrap_err();
        assert_eq!(err, RpcError::Server("no such field".into()));
    }

    #[test]
    fn rejects_legacy_servers() {
        let body = r#"{"arguments":{},"result":"method name not recognized"}"#;
        let err = parse_response::<SessionGet>(body).unwrap_err();
        assert!(matches!(err, RpcError::Protocol(msg) if msg.contains("4.1")));
    }

    /// Against a real daemon:
    /// `TRANSMISSION_URL=http://nas:9091/transmission/rpc [TRANSMISSION_USER=… TRANSMISSION_PASSWORD=…]
    /// cargo test -p cosmic-applet-transmission -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "needs a Transmission daemon"]
    async fn live_server() {
        let url = std::env::var("TRANSMISSION_URL").expect("TRANSMISSION_URL");
        let user = std::env::var("TRANSMISSION_USER").unwrap_or_default();
        let client = Client::new(url, &user, std::env::var("TRANSMISSION_PASSWORD").ok());
        match client.snapshot().await {
            Ok(snapshot) => println!(
                "{} torrents, alt speed {}",
                snapshot.torrents.len(),
                snapshot.alt_speed
            ),
            Err(err) => println!("error: {err}"),
        }
    }
}
