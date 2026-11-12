use std::{net::SocketAddr, time::Instant};

use axum::{
    body::{Body, HttpBody},
    extract::{ConnectInfo, Request},
    http::{HeaderMap, Method, header},
    middleware::Next,
    response::Response,
};

use crate::server::HEALTHZ_PATH;

/// Per-request access log: one `info` event per request, except health checks.
pub struct AccessLog;

impl AccessLog {
    /// axum middleware (use with `axum::middleware::from_fn`).
    pub async fn middleware(request: Request, next: Next) -> Response {
        if request.uri().path() == HEALTHZ_PATH {
            return next.run(request).await;
        }

        let entry = AccessLogEntry::from_request(&request);
        let start = Instant::now();
        let response = next.run(request).await;
        entry.record(&response, start);
        response
    }
}

struct AccessLogEntry {
    method: Method,
    path: String,
    referrer: Option<String>,
    user_agent: Option<String>,
    client: Option<String>,
}

impl AccessLogEntry {
    fn from_request(request: &Request) -> Self {
        let headers = request.headers();
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip().to_string());
        Self {
            method: request.method().clone(),
            path: request.uri().path().to_owned(),
            referrer: header_str(headers, header::REFERER),
            user_agent: header_str(headers, header::USER_AGENT),
            client: forwarded_for(headers).or(peer),
        }
    }

    fn record(self, response: &Response, start: Instant) {
        let status = response.status().as_u16();
        let bytes = self.response_bytes(response);
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        tracing::info!(
            target: "aphid_server::access",
            method = %self.method,
            path = %self.path,
            status,
            duration_ms,
            bytes,
            referrer = self.referrer.as_deref().unwrap_or(""),
            user_agent = self.user_agent.as_deref().unwrap_or(""),
            client = self.client.as_deref().unwrap_or(""),
            "request"
        );
    }

    /// Body bytes sent: `Content-Length` where present, the body's exact size
    /// otherwise, and zero for `HEAD` (which never carries a body).
    fn response_bytes(&self, response: &Response<Body>) -> u64 {
        if self.method == Method::HEAD {
            return 0;
        }
        response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .or_else(|| response.body().size_hint().exact())
            .unwrap_or(0)
    }
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// The original client: the left-most entry of `X-Forwarded-For`.
fn forwarded_for(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .find(|v| !v.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_for_takes_left_most_entry() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        headers.append("x-forwarded-for", "10.0.0.2".parse().unwrap());
        assert_eq!(forwarded_for(&headers).as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn forwarded_for_absent() {
        assert_eq!(forwarded_for(&HeaderMap::new()), None);
    }
}
