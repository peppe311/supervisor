use serde::Serialize;
use url::Url;

pub(crate) const DEFAULT_THEME: &str = "central";
pub(crate) const DEFAULT_SEARCH_ENGINE: &str = "duckduckgo";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreferenceOption {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SearchEngine {
    pub id: &'static str,
    pub name: &'static str,
    action: &'static str,
    query_parameter: &'static str,
}

pub(crate) const SEARCH_ENGINES: [SearchEngine; 11] = [
    SearchEngine {
        id: "google",
        name: "Google",
        action: "https://www.google.com/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "bing",
        name: "Microsoft Bing",
        action: "https://www.bing.com/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "duckduckgo",
        name: "DuckDuckGo",
        action: "https://duckduckgo.com/",
        query_parameter: "q",
    },
    SearchEngine {
        id: "brave",
        name: "Brave Search",
        action: "https://search.brave.com/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "startpage",
        name: "Startpage",
        action: "https://www.startpage.com/sp/search",
        query_parameter: "query",
    },
    SearchEngine {
        id: "ecosia",
        name: "Ecosia",
        action: "https://www.ecosia.org/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "qwant",
        name: "Qwant",
        action: "https://www.qwant.com/",
        query_parameter: "q",
    },
    SearchEngine {
        id: "yahoo",
        name: "Yahoo",
        action: "https://search.yahoo.com/search",
        query_parameter: "p",
    },
    SearchEngine {
        id: "kagi",
        name: "Kagi",
        action: "https://kagi.com/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "mojeek",
        name: "Mojeek",
        action: "https://www.mojeek.com/search",
        query_parameter: "q",
    },
    SearchEngine {
        id: "yandex",
        name: "Yandex",
        action: "https://yandex.com/search/",
        query_parameter: "text",
    },
];

pub(crate) fn normalize_theme(value: &str) -> &'static str {
    match value.trim() {
        "central_dark" | "dark" => "central_dark",
        _ => DEFAULT_THEME,
    }
}

pub(crate) fn theme_surface_color(value: &str) -> (u8, u8, u8, u8) {
    let color = match normalize_theme(value) {
        "central_dark" => 0x0d0d0d,
        _ => 0xeeeeec,
    };
    (
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
        0xff,
    )
}

pub(crate) fn search_engine(value: &str) -> &'static SearchEngine {
    SEARCH_ENGINES
        .iter()
        .find(|engine| engine.id == value)
        .unwrap_or_else(|| {
            SEARCH_ENGINES
                .iter()
                .find(|engine| engine.id == DEFAULT_SEARCH_ENGINE)
                .expect("the default search engine must exist")
        })
}

pub(crate) fn search_engine_options() -> Vec<PreferenceOption> {
    SEARCH_ENGINES
        .iter()
        .map(|engine| PreferenceOption {
            id: engine.id,
            name: engine.name,
        })
        .collect()
}

impl SearchEngine {
    pub(crate) fn search_url(&self, query: &str) -> String {
        let mut url = Url::parse(self.action).expect("search engine URL must be valid");
        url.query_pairs_mut()
            .append_pair(self.query_parameter, query);
        url.into()
    }

    pub(crate) fn action(&self) -> &'static str {
        self.action
    }

    pub(crate) fn query_parameter(&self) -> &'static str {
        self.query_parameter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_two_modes_for_one_product_identity() {
        assert_eq!(DEFAULT_THEME, "central");
        assert_eq!(normalize_theme("light"), "central");
        assert_eq!(normalize_theme("dark"), "central_dark");
        assert_eq!(normalize_theme("central_dark"), "central_dark");
    }

    #[test]
    fn falls_back_to_safe_defaults() {
        assert_eq!(normalize_theme("unknown"), DEFAULT_THEME);
        assert_eq!(search_engine("unknown").id, DEFAULT_SEARCH_ENGINE);
        assert_eq!(theme_surface_color("unknown"), (238, 238, 236, 255));
        assert_eq!(theme_surface_color("dark"), (13, 13, 13, 255));
    }

    #[test]
    fn builds_provider_specific_search_urls() {
        assert_eq!(
            search_engine("google").search_url("rust wasm"),
            "https://www.google.com/search?q=rust+wasm"
        );
        assert_eq!(
            search_engine("yahoo").search_url("central agent"),
            "https://search.yahoo.com/search?p=central+agent"
        );
    }
}
