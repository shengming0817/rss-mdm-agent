# Native acceptance WebDriver source

MIT upstream tauri-plugin-wdio-webdriver 1.4.0, fixed source and original file hashes in [upstream.json](upstream.json). The [upstream license](LICENSE) is retained.

Local changes: `src/lib.rs` requires a pre-bound listener and per-run capability; `src/server/mod.rs` authenticates all HTTP requests before routing, including unknown routes. `Cargo.toml` adds constant_time_eq for token comparison and enables Tokio io-util for HTTP authentication tests. An isolated Cargo workspace marker supports nested worktrees. No unauthenticated initializer is retained. Upstream README examples describe the original API, not this secured initializer.

`src/server/handlers/window.rs` requests native `close()` instead of forced `destroy()`, so the close/reopen acceptance path observes the same CloseRequested lifecycle as the window control.

README links are fixed to the upstream revision; the product formatter normalizes upstream Rust whitespace without changing behavior.

Only the desktop native-e2e feature consumes this dependency. Release builds reject that feature. No product IPC or model/permission mocks are added.
