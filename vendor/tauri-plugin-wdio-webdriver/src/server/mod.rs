use std::net::TcpListener;
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};
use tokio::runtime::Runtime as TokioRuntime;
use tokio::sync::RwLock;

pub mod handlers;
pub mod response;
pub mod router;

use crate::platform::{create_executor, FrameId, PlatformExecutor};
use crate::server::response::WebDriverErrorResponse;
use crate::webdriver::{SessionManager, Timeouts};

/// Shared state for the `WebDriver` server
pub struct AppState<R: Runtime> {
    pub app: AppHandle<R>,
    pub sessions: RwLock<SessionManager>,
}

impl<R: Runtime + 'static> AppState<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            sessions: RwLock::new(SessionManager::new()),
        }
    }

    /// Get a platform executor for a specific window by label
    pub fn get_executor_for_window(
        &self,
        window_label: &str,
        timeouts: Timeouts,
        frame_context: Vec<FrameId>,
    ) -> Result<Arc<dyn PlatformExecutor<R>>, WebDriverErrorResponse> {
        self.app
            .webview_windows()
            .get(window_label)
            .cloned()
            .map(|window| create_executor(window, timeouts, frame_context))
            .ok_or_else(WebDriverErrorResponse::no_such_window)
    }

    /// Whether a window with this label is still registered
    pub fn has_window_label(&self, window_label: &str) -> bool {
        self.app.webview_windows().contains_key(window_label)
    }

    /// Get all window labels
    pub fn get_window_labels(&self) -> Vec<String> {
        self.app.webview_windows().keys().cloned().collect()
    }
}

/// Start the `WebDriver` HTTP server on the specified port
pub fn start<R: Runtime + 'static>(app: AppHandle<R>, listener: TcpListener, capability: [u8; 16]) {
    std::thread::spawn(move || {
        let rt = match TokioRuntime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create Tokio runtime for WebDriver server: {}", e);
                return;
            }
        };

        rt.block_on(async {
            let state = Arc::new(AppState::new(app));
            let router = authenticate(router::create_router(state), capability);
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::error!("Failed to adopt WebDriver listener: {}", error);
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!("WebDriver server error: {}", e);
            }
        });
    });
}

// ref: axum 0.8.9 src/middleware/from_fn.rs (authentication example).
// Layer the whole router, not selected endpoints: status, session, script,
// extension routes, and fallbacks all require the same capability.
fn authenticate(router: axum::Router, capability: [u8; 16]) -> axum::Router {
    use axum::{
        extract::{Request, State},
        http::{header::AUTHORIZATION, StatusCode},
        middleware::{self, Next},
        response::Response,
    };
    let expected = format!(
        "Bearer {}",
        capability
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    async fn authorize(
        State(expected): State<Arc<String>>,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let mut headers = request.headers().get_all(AUTHORIZATION).iter();
        let valid = headers.next().is_some_and(|value| {
            constant_time_eq::constant_time_eq(value.as_bytes(), expected.as_bytes())
        }) && headers.next().is_none();
        if !valid {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(next.run(request).await)
    }
    router.layer(middleware::from_fn_with_state(
        Arc::new(expected),
        authorize,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::any, Router};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn every_request_requires_the_exact_run_capability() {
        let router = authenticate(
            Router::new().route("/{*path}", any(|| async { "reached handler" })),
            [0xab; 16],
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        async fn status(
            address: std::net::SocketAddr,
            method: &str,
            path: &str,
            authorization: &str,
        ) -> String {
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            socket.write_all(format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n{authorization}\r\n").as_bytes()).await.unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).await.unwrap();
            response.lines().next().unwrap().to_owned()
        }
        for path in [
            "/status",
            "/session",
            "/session/one/execute/sync",
            "/wdio/eval",
            "/unknown",
        ] {
            for method in ["GET", "POST", "DELETE", "OPTIONS"] {
                for token in [
                    None,
                    Some("Bearer 00000000000000000000000000000000"),
                    Some("Bearer abababababababababababababababa"),
                    Some("Basic abababababababababababababababab"),
                ] {
                    let authorization = token
                        .map(|token| format!("Authorization: {token}\r\n"))
                        .unwrap_or_default();
                    assert_eq!(
                        status(address, method, path, &authorization).await,
                        "HTTP/1.1 401 Unauthorized"
                    );
                }
                assert_eq!(
                    status(
                        address,
                        method,
                        path,
                        "Authorization: Bearer abababababababababababababababab\r\n"
                    )
                    .await,
                    "HTTP/1.1 200 OK"
                );
            }
        }
        assert_eq!(status(address, "GET", "/status", "Authorization: Bearer abababababababababababababababab\r\nAuthorization: Bearer wrong\r\n").await, "HTTP/1.1 401 Unauthorized");
        server.abort();
        let _ = server.await;
    }
}
