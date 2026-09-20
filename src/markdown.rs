use std::collections::{HashMap, HashSet};

use ammonia::{Builder, UrlRelative};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

pub(crate) fn render_safe_markdown(source: &str) -> String {
    if source.is_empty() {
        return String::new();
    }

    let options = Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(value) | Event::InlineHtml(value) => Event::Text(value),
        Event::Start(Tag::Image { .. }) => Event::Start(Tag::Emphasis),
        Event::End(TagEnd::Image) => Event::End(TagEnd::Emphasis),
        event => event,
    });
    let mut generated = String::with_capacity(source.len().saturating_add(64));
    html::push_html(&mut generated, events);

    let tags = HashSet::from([
        "a",
        "blockquote",
        "br",
        "code",
        "del",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "input",
        "li",
        "ol",
        "p",
        "pre",
        "strong",
        "span",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "tr",
        "ul",
    ]);
    let attributes = HashMap::from([
        ("a", HashSet::from(["href", "title"])),
        ("code", HashSet::from(["class"])),
        ("input", HashSet::from(["checked", "disabled", "type"])),
        ("li", HashSet::from(["class"])),
        ("ol", HashSet::from(["start"])),
        ("span", HashSet::from(["class"])),
        ("td", HashSet::from(["align", "colspan", "rowspan"])),
        (
            "th",
            HashSet::from(["align", "colspan", "rowspan", "scope"]),
        ),
    ]);
    let schemes = HashSet::from(["http", "https"]);
    let mut sanitizer = Builder::new();
    sanitizer
        .tags(tags)
        .tag_attributes(attributes)
        .generic_attributes(HashSet::new())
        .url_schemes(schemes)
        .url_relative(UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank");
    sanitizer.clean(&generated).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_model_html_and_removes_unsafe_links_and_images() {
        let rendered = render_safe_markdown(
            "<script>alert('x')</script>\n\n[unsafe](javascript:alert(1)) [safe](https://example.com) ![remote](https://example.com/a.png)",
        );

        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;"));
        assert!(!rendered.contains("href=\"javascript:"));
        assert!(!rendered.contains("<img"));
        assert!(
            rendered.contains("href=\"https://example.com\""),
            "{rendered}"
        );
        assert!(rendered.contains("target=\"_blank\""), "{rendered}");
        assert!(
            rendered.contains("rel=\"noopener noreferrer\""),
            "{rendered}"
        );
    }

    #[test]
    fn safely_renders_an_incomplete_streaming_fragment() {
        let rendered = render_safe_markdown("Starting **the final");

        assert!(rendered.contains("Starting **the final"));
        assert!(!rendered.contains("<script"));
    }

    #[test]
    fn renders_inline_and_display_math_without_raw_html() {
        let rendered = render_safe_markdown("Inline $x^2 + y^2$ and display:\n\n$$E = mc^2$$");

        assert!(
            rendered.contains("class=\"math math-inline\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("class=\"math math-display\""),
            "{rendered}"
        );
        assert!(rendered.contains("E = mc^2"), "{rendered}");
    }
}
