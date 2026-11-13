//! A small blocking HTTP layer over ureq. Every call runs on a worker thread (`net::pool`), never on
//! the UI thread; the answer reaches app state through `AppHandle::update`.
use std::sync::OnceLock;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Why a request failed, in the terms the UI shows.
#[derive(Clone, Debug, PartialEq)]
pub enum NetError {
    /// The token was refused (HTTP 401): sign in again, or refresh first.
    Unauthorized,
    /// No route to the server: offline, DNS, TLS or a timeout.
    Offline(String),
    /// The server answered with an error status and a message.
    Status(u16, String),
    /// The body wasn't what we expected.
    Decode(String),
}

impl NetError {
    pub fn is_offline(&self) -> bool { matches!(self, NetError::Offline(_)) }
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Unauthorized => f.write_str("Your session expired. Sign in again."),
            NetError::Offline(_) => f.write_str("Can't reach Turbodoc. You're working offline."),
            NetError::Status(code, msg) if msg.is_empty() => write!(f, "The server answered {code}."),
            NetError::Status(_, msg) => f.write_str(msg),
            NetError::Decode(why) => write!(f, "Unexpected answer from the server ({why})."),
        }
    }
}

pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(8)))
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(concat!("Turbodoc-Desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

/// One request: method, absolute URL, headers and an optional JSON body.
pub struct Request<'a> {
    pub method: Method,
    pub url: &'a str,
    pub headers: Vec<(&'a str, String)>,
    pub body: Option<Value>,
}

impl<'a> Request<'a> {
    pub fn new(method: Method, url: &'a str) -> Self { Self { method, url, headers: Vec::new(), body: None } }
    pub fn header(mut self, name: &'a str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
    pub fn bearer(self, token: &str) -> Self { self.header("Authorization", format!("Bearer {token}")) }
    pub fn json(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Sends the request and returns the status and body text.
    pub fn send_raw(self) -> Result<(u16, String), NetError> {
        let a = agent();
        macro_rules! with_headers {
            ($req:expr) => {{
                let mut r = $req;
                for (k, v) in &self.headers {
                    r = r.header(*k, v.as_str());
                }
                r.header("Accept", "application/json")
            }};
        }
        let result = match self.method {
            Method::Get => with_headers!(a.get(self.url)).call(),
            Method::Delete => with_headers!(a.delete(self.url)).call(),
            Method::Post | Method::Put => {
                let r = if self.method == Method::Post { with_headers!(a.post(self.url)) } else { with_headers!(a.put(self.url)) };
                match &self.body {
                    Some(body) => r.header("Content-Type", "application/json").send(body.to_string().as_bytes()),
                    None => r.send_empty(),
                }
            }
        };
        let mut response = result.map_err(|e| NetError::Offline(e.to_string()))?;
        let status = response.status().as_u16();
        let text = response.body_mut().with_config().limit(32 * 1024 * 1024).read_to_string().map_err(|e| NetError::Offline(e.to_string()))?;
        Ok((status, text))
    }

    /// Sends the request and decodes a JSON answer; error statuses become [`NetError`].
    pub fn send<T: DeserializeOwned>(self) -> Result<T, NetError> {
        let (status, text) = self.send_raw()?;
        check(status, &text)?;
        let text = if text.trim().is_empty() { "null" } else { text.as_str() };
        serde_json::from_str(text).map_err(|e| NetError::Decode(e.to_string()))
    }
}

/// Fetches raw bytes (images), bounded to `limit`.
pub fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, NetError> {
    let mut response = agent().get(url).header("Accept", "image/*,*/*;q=0.8").call().map_err(|e| NetError::Offline(e.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(NetError::Status(status, String::new()));
    }
    response.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| NetError::Offline(e.to_string()))
}

/// Maps an HTTP status and body to an error, reading `message`, `error_description`, `msg` or
/// `error` from a JSON body (the API's and Supabase Auth's shapes).
pub fn check(status: u16, body: &str) -> Result<(), NetError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 401 {
        return Err(NetError::Unauthorized);
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            ["message", "error_description", "msg", "error"]
                .iter()
                .find_map(|k| v.get(*k).and_then(Value::as_str).map(str::to_owned))
        })
        .unwrap_or_default();
    Err(NetError::Status(status, message))
}

/// Percent-encodes a query value.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
