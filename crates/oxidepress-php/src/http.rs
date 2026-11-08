// Modified by Vabian124 on 2026-11-08 to let the PHP worker make public HTTP requests.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Public HTTP for the PHP worker.
//!
//! Plugins call `wp_remote_get` and `wp_remote_post`. Private addresses,
//! internal hostnames, and non-HTTP schemes are refused before any socket
//! opens. Redirects are not followed, so a public URL cannot bounce to a
//! private one.

use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

use serde_json::{json, Value};

const MAX_URL_LEN: usize = 2048;
const MAX_BODY_LEN: usize = 1024 * 1024;
const MAX_HEADERS: usize = 32;

/// One outbound request from the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub timeout_secs: u64,
}

/// A completed response. Transport failures are `Err`, not a status code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: String,
    pub body: String,
}

/// `Some(reason)` when the URL must not be fetched.
pub fn unsafe_url(raw: &str) -> Option<String> {
    if raw.len() > MAX_URL_LEN {
        return Some("URL is too long".to_string());
    }
    let parsed = match url::Url::parse(raw) {
        Ok(parsed) => parsed,
        Err(error) => return Some(format!("Invalid URL: {error}")),
    };
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => return Some(format!("Blocked scheme: {scheme}")),
    }
    match parsed.host() {
        Some(url::Host::Domain(host)) => {
            let lower = host.to_ascii_lowercase();
            if lower == "localhost"
                || lower == "internal"
                || lower.ends_with(".localhost")
                || lower.ends_with(".local")
                || lower.ends_with(".internal")
            {
                return Some(format!("Blocked internal hostname: {host}"));
            }
            None
        }
        Some(url::Host::Ipv4(ip)) if is_private_ip(IpAddr::V4(ip)) => {
            Some(format!("Blocked private IP: {ip}"))
        }
        Some(url::Host::Ipv6(ip)) if is_private_ip(IpAddr::V6(ip)) => {
            Some(format!("Blocked private IP: {ip}"))
        }
        Some(_) => None,
        None => Some("No host in URL".to_string()),
    }
}

pub fn perform(request: &HttpRequest) -> Result<HttpResponse, String> {
    if let Some(reason) = unsafe_url(&request.url) {
        return Err(reason);
    }
    if request.body.len() > MAX_BODY_LEN {
        return Err("request body exceeded 1 MiB".to_string());
    }
    let method = normalize_method(&request.method)?;
    reject_resolved(&request.url)?;
    let timeout = Duration::from_secs(request.timeout_secs.clamp(1, 15));
    let agent = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(0)
        .build();
    let mut call = agent.request(&method, &request.url);
    for (name, value) in request.headers.iter().take(MAX_HEADERS) {
        if header_allowed(name, value) {
            call = call.set(name, value);
        }
    }
    let result = if method == "GET" || method == "HEAD" || request.body.is_empty() {
        call.call()
    } else {
        call.send_string(&request.body)
    };
    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.to_string()),
    };
    let status = response.status();
    let reason = response.status_text().to_string();
    let mut reader = response.into_reader().take(MAX_BODY_LEN as u64 + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read response: {error}"))?;
    if bytes.len() > MAX_BODY_LEN {
        return Err("response exceeded 1 MiB".to_string());
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();
    Ok(HttpResponse {
        status,
        reason,
        body,
    })
}

pub fn dispatch(host: &dyn crate::PhpHost, frame: &Value) -> Value {
    let url = frame.get("url").and_then(Value::as_str).unwrap_or("");
    if let Some(reason) = unsafe_url(url) {
        return json!({ "error": reason });
    }
    let method = frame.get("method").and_then(Value::as_str).unwrap_or("GET");
    let body = frame.get("body").and_then(Value::as_str).unwrap_or("");
    if body.len() > MAX_BODY_LEN {
        return json!({ "error": "request body exceeded 1 MiB" });
    }
    let timeout = frame
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 15);
    let mut headers = Vec::new();
    if let Some(object) = frame.get("headers").and_then(Value::as_object) {
        for (name, value) in object {
            let Some(text) = value.as_str() else {
                continue;
            };
            if header_allowed(name, text) {
                headers.push((name.clone(), text.to_string()));
            }
            if headers.len() == MAX_HEADERS {
                break;
            }
        }
    }
    let request = HttpRequest {
        method: method.to_string(),
        url: url.to_string(),
        headers,
        body: body.to_string(),
        timeout_secs: timeout,
    };
    match host.http_request(&request) {
        Ok(response) => json!({
            "status": response.status,
            "reason": response.reason,
            "body": response.body,
            "error": Value::Null,
        }),
        Err(error) => json!({ "error": error }),
    }
}

fn normalize_method(method: &str) -> Result<String, String> {
    let method = method.trim().to_ascii_uppercase();
    match method.as_str() {
        "GET" | "POST" | "HEAD" | "PUT" | "DELETE" => Ok(method),
        _ => Err(format!("blocked HTTP method: {method}")),
    }
}

fn header_allowed(name: &str, value: &str) -> bool {
    if name.is_empty()
        || name
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii() || *byte < 32)
        || value
            .as_bytes()
            .iter()
            .any(|byte| *byte == b'\r' || *byte == b'\n')
    {
        return false;
    }
    !matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "content-length"
            | "transfer-encoding"
            | "upgrade"
            | "proxy-connection"
            | "proxy-authorization"
    )
}

fn reject_resolved(raw: &str) -> Result<(), String> {
    let parsed = url::Url::parse(raw).map_err(|error| format!("Invalid URL: {error}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "No host in URL".to_string())?;
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let port = parsed.port_or_known_default().unwrap_or(80);
    let mut saw_address = false;
    for addr in (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("DNS failed: {error}"))?
    {
        saw_address = true;
        if is_private_ip(addr.ip()) {
            return Err(format!("DNS resolved to private IP: {}", addr.ip()));
        }
    }
    if !saw_address {
        return Err("DNS returned no addresses".to_string());
    }
    Ok(())
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [first, second, _, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.is_documentation()
                || (first == 100 && (second & 0xc0) == 64)
        }
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_private_ip(IpAddr::V4(mapped)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_https_url_is_allowed() {
        assert!(unsafe_url("https://example.com/1.1/comment-check").is_none());
        assert!(unsafe_url("https://rest.akismet.com/1.1/comment-check").is_none());
    }

    #[test]
    fn private_and_non_http_urls_are_blocked() {
        assert!(unsafe_url("http://127.0.0.1/secret")
            .unwrap()
            .contains("private"));
        assert!(unsafe_url("http://169.254.169.254/latest/meta-data/")
            .unwrap()
            .contains("private"));
        assert!(unsafe_url("http://10.0.0.1/internal")
            .unwrap()
            .contains("private"));
        assert!(unsafe_url("http://localhost/secret")
            .unwrap()
            .contains("hostname"));
        assert!(unsafe_url("http://db.internal/query")
            .unwrap()
            .contains("hostname"));
        assert!(unsafe_url("file:///etc/passwd").unwrap().contains("scheme"));
        assert!(unsafe_url("ftp://example.com/file")
            .unwrap()
            .contains("scheme"));
    }
}
