use url::Url;

use crate::preferences::SearchEngine;

pub fn normalize_address_input(value: &str, search_engine: &SearchEngine) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(url) = Url::parse(value)
        && is_supported_url(url.as_str())
    {
        return Some(url.into());
    }

    if looks_like_host(value)
        && let Ok(url) = Url::parse(&format!("https://{value}"))
        && is_supported_url(url.as_str())
    {
        return Some(url.into());
    }

    Some(search_engine.search_url(value))
}

pub fn is_supported_url(value: &str) -> bool {
    match Url::parse(value) {
        Ok(url) => matches!(url.scheme(), "http" | "https"),
        Err(_) => false,
    }
}

pub fn is_allowed_webview_navigation(value: &str) -> bool {
    if value == "about:blank" {
        return true;
    }

    match Url::parse(value) {
        Ok(url) => matches!(url.scheme(), "http" | "https" | "data" | "blob"),
        Err(_) => false,
    }
}

pub fn address_for_display(value: &str) -> String {
    if is_supported_url(value) {
        value.to_owned()
    } else {
        String::new()
    }
}

fn looks_like_host(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }

    value.contains('.')
        || value.starts_with("localhost")
        || value.parse::<std::net::IpAddr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::search_engine;

    #[test]
    fn preserves_supported_urls() {
        assert_eq!(
            normalize_address_input("https://example.com/docs", search_engine("duckduckgo")),
            Some("https://example.com/docs".to_owned())
        );
        assert_eq!(
            normalize_address_input("http://localhost:3000", search_engine("duckduckgo")),
            Some("http://localhost:3000/".to_owned())
        );
    }

    #[test]
    fn adds_https_to_hosts() {
        assert_eq!(
            normalize_address_input("example.com/path", search_engine("duckduckgo")),
            Some("https://example.com/path".to_owned())
        );
    }

    #[test]
    fn turns_plain_text_into_a_search() {
        let result =
            normalize_address_input("rust webview browser", search_engine("duckduckgo")).unwrap();
        assert_eq!(result, "https://duckduckgo.com/?q=rust+webview+browser");
    }

    #[test]
    fn uses_the_selected_search_engine() {
        let result =
            normalize_address_input("rust webview browser", search_engine("google")).unwrap();
        assert_eq!(
            result,
            "https://www.google.com/search?q=rust+webview+browser"
        );
    }

    #[test]
    fn rejects_privileged_schemes() {
        assert!(!is_supported_url("file:///C:/Windows/System32/config/SAM"));
        assert!(!is_supported_url("javascript:alert(1)"));
    }

    #[test]
    fn allows_internal_blank_document_only() {
        assert!(is_allowed_webview_navigation("about:blank"));
        assert!(is_allowed_webview_navigation(
            "data:text/html,<h1>New tab</h1>"
        ));
        assert!(is_allowed_webview_navigation(
            "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!is_allowed_webview_navigation("about:config"));
        assert!(!is_allowed_webview_navigation("file:///C:/secrets.txt"));
    }
}
