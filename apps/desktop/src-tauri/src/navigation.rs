// ref: Tauri crates/tauri/src/webview/webview_window.rs@tauri-v2.11.2
use tauri::Url;

pub fn allowed(url: &Url) -> bool {
    allowed_in_mode(url, cfg!(dev), cfg!(target_os = "windows"))
}

fn allowed_in_mode(url: &Url, development: bool, windows: bool) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let app_origin = if windows {
        ("http", Some("tauri.localhost"), None)
    } else {
        ("tauri", Some("localhost"), None)
    };
    ((url.scheme(), url.host_str(), url.port()) == app_origin)
        || (development
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port() == Some(1420))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_is_limited_to_exact_app_or_development_origins() {
        for (url, windows) in [
            ("tauri://localhost/index.html", false),
            ("http://tauri.localhost/index.html", true),
        ] {
            let url = Url::parse(url).unwrap();
            assert!(allowed_in_mode(&url, false, windows));
            assert!(!allowed_in_mode(&url, false, !windows));
        }
        let development = Url::parse("http://127.0.0.1:1420/").unwrap();
        assert!(allowed_in_mode(
            &development,
            true,
            cfg!(target_os = "windows")
        ));
        assert!(!allowed_in_mode(
            &development,
            false,
            cfg!(target_os = "windows")
        ));
        for url in [
            "https://example.com/",
            "https://tauri.localhost/",
            "https://tauri.localhost.evil.test/",
            "tauri://evil/index.html",
            "http://tauri.localhost:1420/",
            "http://127.0.0.1:1421/",
            "http://localhost:1420/",
            "https://127.0.0.1:1420/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hello",
            "http://user@tauri.localhost/",
            "http://user:secret@127.0.0.1:1420/",
            "about:blank",
        ] {
            for (development, windows) in
                [(true, true), (true, false), (false, true), (false, false)]
            {
                assert!(
                    !allowed_in_mode(&Url::parse(url).unwrap(), development, windows),
                    "{url}"
                );
            }
        }
    }
}
