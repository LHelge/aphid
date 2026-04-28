use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast;

use super::AppState;

/// Handle a WebSocket upgrade request for live reload.
pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.reload_tx.subscribe();
    // Pinned across iterations: see comment on `ContentWatcher::run` —
    // a fresh `notified()` per loop would race with `notify_waiters()`.
    let shutdown = state.shutdown.notified();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            result = rx.recv() => {
                match result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        if socket.send(Message::Text("reload".into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }
}

/// The live-reload script injected into HTML responses in serve mode.
///
/// Opens a WebSocket to the dev server. On receiving any message, reloads the
/// page. On connection close, retries after 1 second so the page recovers
/// after a server restart.
pub const LIVE_RELOAD_SCRIPT: &str = r#"<script>(function(){var ws=new WebSocket("ws://"+location.host+"/ws");ws.onmessage=function(){location.reload()};ws.onclose=function(){setTimeout(function(){location.reload()},1000)}})()</script>"#;

/// Inject the live-reload script before `</body>` in an HTML string.
/// If `</body>` is not found, appends the script at the end.
pub fn inject_live_reload(html: &str) -> String {
    let mut result = String::with_capacity(html.len() + LIVE_RELOAD_SCRIPT.len());
    match html.rfind("</body>") {
        Some(pos) => {
            result.push_str(&html[..pos]);
            result.push_str(LIVE_RELOAD_SCRIPT);
            result.push_str(&html[pos..]);
        }
        None => {
            result.push_str(html);
            result.push_str(LIVE_RELOAD_SCRIPT);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_before_closing_body() {
        let html = "<html><body><p>Hello</p></body></html>";
        let result = inject_live_reload(html);
        assert!(result.contains(&format!("{LIVE_RELOAD_SCRIPT}</body>")));
        assert!(result.contains("<p>Hello</p>"));
    }

    #[test]
    fn appends_when_no_body_tag() {
        let html = "<p>No body tag</p>";
        let result = inject_live_reload(html);
        assert!(result.ends_with(LIVE_RELOAD_SCRIPT));
    }

    #[test]
    fn script_contains_websocket() {
        assert!(LIVE_RELOAD_SCRIPT.contains("WebSocket"));
        assert!(LIVE_RELOAD_SCRIPT.contains("location.reload()"));
    }
}
