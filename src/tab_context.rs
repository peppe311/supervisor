use std::sync::OnceLock;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageFormat, codecs::jpeg::JpegEncoder, imageops::FilterType};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

pub(crate) const CONTEXT_STALE_AFTER_MS: u128 = 5 * 60 * 1_000;
pub(crate) const MAX_CONTEXT_TEXT_CHARS: usize = 24_000;
pub(crate) const MAX_CONTEXT_ELEMENTS: usize = 80;
pub(crate) const MAX_CONTEXT_LINKS: usize = 120;
pub(crate) const MAX_CONTEXT_IMAGES: usize = 80;
const MAX_TITLE_CHARS: usize = 180;
const MAX_DESCRIPTION_CHARS: usize = 520;
const MAX_ELEMENT_NAME_CHARS: usize = 140;
const MAX_LINK_TEXT_CHARS: usize = 160;
const MAX_IMAGE_ALT_CHARS: usize = 160;
const MAX_RESOURCE_URL_CHARS: usize = 512;
const MAX_CONTEXT_VISUAL_BYTES: usize = 1_500_000;
const VISUAL_THUMBNAIL_WIDTH: u32 = 480;
const VISUAL_THUMBNAIL_HEIGHT: u32 = 270;
const MAX_VISUAL_THUMBNAIL_BYTES: usize = 96_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TabContextStatus {
    Capturing,
    Ready,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContextElement {
    pub role: String,
    pub name: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContextLink {
    pub text: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContextImage {
    pub alt: String,
    pub url: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct TabContextSnapshot {
    pub tab_id: u64,
    pub capture_id: u64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub language: String,
    pub status: TabContextStatus,
    pub error: Option<String>,
    pub captured_at_ms: Option<u128>,
    pub source_revision: u64,
    pub text: String,
    pub text_char_count: usize,
    pub elements: Vec<TabContextElement>,
    pub source_element_count: usize,
    pub links: Vec<TabContextLink>,
    pub source_link_count: usize,
    pub images: Vec<TabContextImage>,
    pub source_image_count: usize,
    pub page_width: u32,
    pub page_height: u32,
    pub visual_jpeg: Option<Vec<u8>>,
    pub visual_thumbnail_data_url: Option<String>,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabContextView {
    pub tab_id: u64,
    pub title: String,
    pub url: String,
    pub status: TabContextStatus,
    pub error: Option<String>,
    pub captured_at_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_data_url: Option<String>,
    pub text_char_count: usize,
    pub element_count: usize,
    pub source_element_count: usize,
    pub link_count: usize,
    pub source_link_count: usize,
    pub image_count: usize,
    pub source_image_count: usize,
    pub visual_included: bool,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    pub truncated: bool,
    pub stale: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawContextPayload {
    title: String,
    url: String,
    description: String,
    language: String,
    text: String,
    elements: Vec<RawContextElement>,
    links: Vec<RawContextLink>,
    images: Vec<RawContextImage>,
    page: RawContextPage,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawContextElement {
    role: String,
    tag: String,
    name: String,
    input_type: String,
    disabled: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawContextLink {
    href: String,
    text: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawContextImage {
    src: String,
    alt: String,
    kind: String,
    width: f64,
    height: f64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawContextPage {
    width: f64,
    height: f64,
}

impl TabContextSnapshot {
    pub(crate) fn capturing(
        tab_id: u64,
        capture_id: u64,
        title: &str,
        url: &str,
        source_revision: u64,
    ) -> Self {
        let (title, title_redactions) = sanitize_limited(title, MAX_TITLE_CHARS);
        Self {
            tab_id,
            capture_id,
            title,
            url: sanitize_context_url(url),
            description: String::new(),
            language: String::new(),
            status: TabContextStatus::Capturing,
            error: None,
            captured_at_ms: None,
            source_revision,
            text: String::new(),
            text_char_count: 0,
            elements: Vec::new(),
            source_element_count: 0,
            links: Vec::new(),
            source_link_count: 0,
            images: Vec::new(),
            source_image_count: 0,
            page_width: 0,
            page_height: 0,
            visual_jpeg: None,
            visual_thumbnail_data_url: None,
            estimated_token_count: 0,
            redaction_count: title_redactions,
            truncated: false,
        }
    }

    pub(crate) fn from_value(
        tab_id: u64,
        capture_id: u64,
        fallback_title: &str,
        fallback_url: &str,
        source_revision: u64,
        captured_at_ms: u128,
        value: Value,
    ) -> Result<Self, String> {
        let raw: RawContextPayload = serde_json::from_value(value)
            .map_err(|error| format!("Invalid page snapshot: {error}"))?;
        let raw_title = if raw.title.trim().is_empty() {
            fallback_title
        } else {
            &raw.title
        };
        let raw_url = if raw.url.trim().is_empty() {
            fallback_url
        } else {
            &raw.url
        };

        let (title, title_redactions) = sanitize_limited(raw_title, MAX_TITLE_CHARS);
        let (description, description_redactions, description_truncated) =
            sanitize_limited_with_truncation(&raw.description, MAX_DESCRIPTION_CHARS);
        let (language, language_truncated) = truncate_chars(raw.language.trim(), 32);
        let (redacted_text, text_redactions) = redact_sensitive(&raw.text);
        let text_char_count = redacted_text.chars().count();
        let (text, text_truncated) = truncate_chars(&redacted_text, MAX_CONTEXT_TEXT_CHARS);
        let source_element_count = raw.elements.len();
        let source_link_count = raw.links.len();
        let source_image_count = raw.images.len();
        let mut redaction_count = title_redactions + description_redactions + text_redactions;
        let mut truncated = text_truncated
            || description_truncated
            || language_truncated
            || source_element_count > MAX_CONTEXT_ELEMENTS
            || source_link_count > MAX_CONTEXT_LINKS
            || source_image_count > MAX_CONTEXT_IMAGES;
        let mut elements = Vec::new();

        for item in raw.elements.into_iter().take(MAX_CONTEXT_ELEMENTS) {
            let raw_role = if item.role.trim().is_empty() {
                item.tag.as_str()
            } else {
                item.role.as_str()
            };
            let (role, role_truncated) = truncate_chars(raw_role.trim(), 48);
            let (name, name_redactions, name_truncated) =
                if item.input_type.eq_ignore_ascii_case("password") {
                    ("[REDACTED FIELD]".to_owned(), 1, false)
                } else {
                    let (redacted, count) = redact_sensitive(&item.name);
                    let (limited, was_truncated) =
                        truncate_chars(redacted.trim(), MAX_ELEMENT_NAME_CHARS);
                    (limited, count, was_truncated)
                };
            redaction_count += name_redactions;
            truncated |= role_truncated || name_truncated;
            elements.push(TabContextElement {
                role,
                name,
                disabled: item.disabled,
            });
        }

        let mut links = Vec::new();
        for item in raw.links.into_iter().take(MAX_CONTEXT_LINKS) {
            let Some(url) = sanitize_resource_url(raw_url, &item.href) else {
                truncated = true;
                continue;
            };
            let (text, text_redactions, text_truncated) =
                sanitize_limited_with_truncation(&item.text, MAX_LINK_TEXT_CHARS);
            redaction_count += text_redactions;
            truncated |= text_truncated;
            links.push(TabContextLink { text, url });
        }

        let mut images = Vec::new();
        for item in raw.images.into_iter().take(MAX_CONTEXT_IMAGES) {
            let Some(url) = sanitize_resource_url(raw_url, &item.src) else {
                truncated = true;
                continue;
            };
            let (alt, alt_redactions, alt_truncated) =
                sanitize_limited_with_truncation(&item.alt, MAX_IMAGE_ALT_CHARS);
            let (kind, kind_truncated) = truncate_chars(item.kind.trim(), 24);
            redaction_count += alt_redactions;
            truncated |= alt_truncated || kind_truncated;
            images.push(TabContextImage {
                alt,
                url,
                kind,
                width: sanitize_dimension(item.width),
                height: sanitize_dimension(item.height),
            });
        }

        let mut snapshot = Self {
            tab_id,
            capture_id,
            title,
            url: sanitize_context_url(raw_url),
            description,
            language,
            status: TabContextStatus::Ready,
            error: None,
            captured_at_ms: Some(captured_at_ms),
            source_revision,
            text,
            text_char_count,
            elements,
            source_element_count,
            links,
            source_link_count,
            images,
            source_image_count,
            page_width: sanitize_dimension(raw.page.width),
            page_height: sanitize_dimension(raw.page.height),
            visual_jpeg: None,
            visual_thumbnail_data_url: None,
            estimated_token_count: 0,
            redaction_count,
            truncated,
        };
        snapshot.estimated_token_count = estimate_snapshot_tokens(&snapshot);
        Ok(snapshot)
    }

    pub(crate) fn fail(&mut self, message: String) {
        self.status = TabContextStatus::Error;
        self.error = Some(message);
        self.captured_at_ms = None;
        self.text.clear();
        self.text_char_count = 0;
        self.elements.clear();
        self.source_element_count = 0;
        self.links.clear();
        self.source_link_count = 0;
        self.images.clear();
        self.source_image_count = 0;
        self.visual_jpeg = None;
        self.visual_thumbnail_data_url = None;
        self.estimated_token_count = 0;
    }

    pub(crate) fn begin_visual_capture(&mut self) {
        self.visual_jpeg = None;
        self.visual_thumbnail_data_url = None;
        self.status = TabContextStatus::Capturing;
    }

    pub(crate) fn finish_visual_capture(&mut self, jpeg: Option<Vec<u8>>, truncated: bool) {
        self.visual_thumbnail_data_url = jpeg.as_deref().and_then(visual_thumbnail_data_url);
        self.visual_jpeg = jpeg;
        self.truncated |= truncated;
        self.status = TabContextStatus::Ready;
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.status == TabContextStatus::Ready
    }

    pub(crate) fn is_capturing(&self) -> bool {
        self.status == TabContextStatus::Capturing
    }

    pub(crate) fn is_stale(&self, current_revision: Option<u64>, now_ms: u128) -> bool {
        if !self.is_ready() {
            return false;
        }
        if current_revision != Some(self.source_revision) {
            return true;
        }
        self.captured_at_ms
            .is_some_and(|captured| now_ms.saturating_sub(captured) > CONTEXT_STALE_AFTER_MS)
    }

    pub(crate) fn view(&self, current_revision: Option<u64>, now_ms: u128) -> TabContextView {
        TabContextView {
            tab_id: self.tab_id,
            title: self.title.clone(),
            url: self.url.clone(),
            status: self.status,
            error: self.error.clone(),
            captured_at_ms: self.captured_at_ms,
            preview_data_url: self.visual_thumbnail_data_url.clone(),
            text_char_count: self.text_char_count,
            element_count: self.elements.len(),
            source_element_count: self.source_element_count,
            link_count: self.links.len(),
            source_link_count: self.source_link_count,
            image_count: self.images.len(),
            source_image_count: self.source_image_count,
            visual_included: self.visual_jpeg.is_some(),
            estimated_token_count: self.estimated_token_count,
            redaction_count: self.redaction_count,
            truncated: self.truncated,
            stale: self.is_stale(current_revision, now_ms),
        }
    }
}

pub(crate) fn sanitize_context_url(value: &str) -> String {
    if let Ok(mut parsed) = Url::parse(value) {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        parsed.set_query(None);
        parsed.set_fragment(None);
        let (limited, _) = truncate_chars(parsed.as_str(), 2_048);
        return limited;
    }
    let (redacted, _) = redact_sensitive(value);
    truncate_chars(&redacted, 2_048).0
}

pub(crate) fn sanitize_context_title(value: &str) -> String {
    sanitize_limited(value, MAX_TITLE_CHARS).0
}

pub(crate) fn sanitize_ui_text(value: &str, max_chars: usize) -> (String, usize) {
    sanitize_limited(value, max_chars)
}

pub(crate) fn sanitize_terminal_snapshot_text(value: &str) -> (String, usize, usize, bool) {
    let (redacted, redaction_count) = redact_sensitive(value);
    let source_char_count = redacted.chars().count();
    (redacted, source_char_count, redaction_count, false)
}

pub(crate) fn decode_visual_capture(value: &str) -> Result<Vec<u8>, String> {
    let data = serde_json::from_str::<Value>(value)
        .map_err(|error| format!("Invalid visual snapshot response: {error}"))?
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| "Visual snapshot did not contain image data".to_owned())?
        .to_owned();
    if data.len() > MAX_CONTEXT_VISUAL_BYTES.saturating_mul(4).div_ceil(3) + 8 {
        return Err("Compressed visual snapshot exceeded the local size limit".to_owned());
    }
    let bytes = STANDARD
        .decode(data)
        .map_err(|error| format!("Invalid visual snapshot image: {error}"))?;
    if bytes.len() > MAX_CONTEXT_VISUAL_BYTES {
        return Err("Compressed visual snapshot exceeded the local size limit".to_owned());
    }
    if !bytes.starts_with(&[0xff, 0xd8, 0xff]) || !bytes.ends_with(&[0xff, 0xd9]) {
        return Err("Visual snapshot was not a valid JPEG image".to_owned());
    }
    Ok(bytes)
}

fn visual_thumbnail_data_url(jpeg: &[u8]) -> Option<String> {
    let source = image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg)
        .ok()?
        .to_rgb8();
    let width = source.width();
    let height = source.height();
    if width == 0 || height == 0 {
        return None;
    }

    let (crop_x, crop_width, crop_height) = if u64::from(width) * 9 > u64::from(height) * 16 {
        let crop_width = ((u64::from(height) * 16) / 9).clamp(1, u64::from(width)) as u32;
        ((width - crop_width) / 2, crop_width, height)
    } else {
        let crop_height = ((u64::from(width) * 9) / 16).clamp(1, u64::from(height)) as u32;
        (0, width, crop_height)
    };
    let crop = image::imageops::crop_imm(&source, crop_x, 0, crop_width, crop_height).to_image();
    let thumbnail = image::imageops::resize(
        &crop,
        VISUAL_THUMBNAIL_WIDTH,
        VISUAL_THUMBNAIL_HEIGHT,
        FilterType::Triangle,
    );
    let thumbnail = DynamicImage::ImageRgb8(thumbnail);

    for quality in [68, 54, 42] {
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, quality)
            .encode_image(&thumbnail)
            .ok()?;
        if encoded.len() <= MAX_VISUAL_THUMBNAIL_BYTES {
            return Some(format!(
                "data:image/jpeg;base64,{}",
                STANDARD.encode(encoded)
            ));
        }
    }
    None
}

fn sanitize_resource_url(base_url: &str, value: &str) -> Option<String> {
    let mut parsed = Url::parse(value)
        .or_else(|_| Url::parse(base_url).and_then(|base| base.join(value)))
        .ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(truncate_chars(parsed.as_str(), MAX_RESOURCE_URL_CHARS).0)
}

fn sanitize_dimension(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.round().clamp(1.0, 100_000.0) as u32
}

fn estimate_snapshot_tokens(snapshot: &TabContextSnapshot) -> usize {
    let fixed_json_overhead = 48;
    let text_tokens = [
        snapshot.title.as_str(),
        snapshot.url.as_str(),
        snapshot.description.as_str(),
        snapshot.language.as_str(),
        snapshot.text.as_str(),
    ]
    .into_iter()
    .map(estimate_text_tokens)
    .sum::<usize>();
    let element_tokens = snapshot
        .elements
        .iter()
        .map(|element| {
            8 + estimate_text_tokens(&element.role) + estimate_text_tokens(&element.name)
        })
        .sum::<usize>();
    let link_tokens = snapshot
        .links
        .iter()
        .map(|link| 8 + estimate_text_tokens(&link.text) + estimate_text_tokens(&link.url))
        .sum::<usize>();
    let image_tokens = snapshot
        .images
        .iter()
        .map(|image| {
            12 + estimate_text_tokens(&image.alt)
                + estimate_text_tokens(&image.url)
                + estimate_text_tokens(&image.kind)
        })
        .sum::<usize>();

    fixed_json_overhead + text_tokens + element_tokens + link_tokens + image_tokens
}

fn estimate_text_tokens(value: &str) -> usize {
    if value.is_empty() {
        return 0;
    }

    let weighted_chars = value.chars().fold(0usize, |total, character| {
        total.saturating_add(if character.is_ascii() { 1 } else { 2 })
    });
    let by_characters = weighted_chars.div_ceil(4);
    let word_count = value.split_whitespace().count();
    let by_words = word_count.saturating_mul(4).div_ceil(3);
    by_characters.max(by_words).max(1)
}

fn sanitize_limited(value: &str, max_chars: usize) -> (String, usize) {
    let (redacted, redactions) = redact_sensitive(value);
    (truncate_chars(redacted.trim(), max_chars).0, redactions)
}

fn sanitize_limited_with_truncation(value: &str, max_chars: usize) -> (String, usize, bool) {
    let (redacted, redactions) = redact_sensitive(value);
    let (limited, truncated) = truncate_chars(redacted.trim(), max_chars);
    (limited, redactions, truncated)
}

pub(crate) fn redact_sensitive(value: &str) -> (String, usize) {
    let mut output = value.to_owned();
    let mut count = 0;
    for (pattern, replacement) in redaction_patterns() {
        let matches = pattern.find_iter(&output).count();
        if matches > 0 {
            output = pattern.replace_all(&output, *replacement).into_owned();
            count += matches;
        }
    }
    (output, count)
}

pub(crate) fn redaction_safe_prefix_len(value: &str, desired: usize) -> usize {
    let mut split = desired.min(value.len());
    while split > 0 && !value.is_char_boundary(split) {
        split -= 1;
    }
    loop {
        let mut adjusted = split;
        for (pattern, _) in redaction_patterns() {
            for found in pattern.find_iter(value) {
                if found.start() < adjusted && found.end() > adjusted {
                    adjusted = found.start();
                }
            }
        }
        if adjusted == split {
            return split;
        }
        split = adjusted;
    }
}

fn redaction_patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            (
                r"(?i)\b(password|passcode|otp|pin|secret|api[_ -]?key|token|access[_ -]?token|auth[_ -]?token|refresh[_ -]?token|session[_ -]?token|gh[_ -]?token|client[_ -]?secret|private[_ -]?key|aws[_ -]?secret[_ -]?access[_ -]?key|authorization|credential)\b[ \t]*[:=][ \t]*[^\r\n]*",
                "$1: [REDACTED DATA]",
            ),
            (
                r"(?i)\bbearer[ \t]+[A-Za-z0-9._~+/=-]{8,}",
                "Bearer [REDACTED TOKEN]",
            ),
            (r"\b(?:sk|pk)-[A-Za-z0-9_-]{8,}\b", "[REDACTED TOKEN]"),
            (
                r"(?i)\b(?:ghp_[A-Za-z0-9]{8,}|github_pat_[A-Za-z0-9_]{8,}|xox[baprs]-[A-Za-z0-9-]{8,}|AKIA[A-Z0-9]{12,})\b",
                "[REDACTED TOKEN]",
            ),
            (
                r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
                "[REDACTED TOKEN]",
            ),
            (
                r"(?i)\b[A-Z0-9._%+-]{1,64}@[A-Z0-9.-]{1,253}\.[A-Z]{2,63}\b",
                "[REDACTED EMAIL]",
            ),
            (
                r"(?i)\b([a-z][a-z0-9+.-]{1,15})://[^/\s:@]{1,256}:[^@\s/]+@",
                "$1://[REDACTED CREDENTIAL]@",
            ),
            (r"\b(?:\d[ -]?){12,19}\b", "[REDACTED NUMBER]"),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(pattern).expect("valid redaction pattern"),
                replacement,
            )
        })
        .collect()
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    let was_truncated = chars.next().is_some();
    (truncated, was_truncated)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_value() -> Value {
        json!({
            "title": "Private area user@example.com",
            "url": "https://example.com/account?token=secret#profile",
            "description": "Private area for user@example.com",
            "language": "en",
            "text": "Email user@example.com password: topsecret card 4111 1111 1111 1111",
            "elements": [
                { "role": "button", "tag": "button", "name": "Continue", "disabled": false },
                { "role": "textbox", "tag": "input", "name": "Current password", "inputType": "password", "disabled": false }
            ],
            "links": [
                { "href": "/docs?token=private#start", "text": "Docs for user@example.com" },
                { "href": "javascript:alert(1)", "text": "Unsafe" }
            ],
            "images": [
                { "src": "/hero.jpg?signature=private", "alt": "Portrait user@example.com", "kind": "image", "width": 1200, "height": 800 },
                { "src": "data:image/png;base64,private", "alt": "Embedded", "kind": "image" }
            ],
            "page": { "width": 1440, "height": 6000 }
        })
    }

    #[test]
    fn sanitizes_snapshot_before_storage() {
        let snapshot = TabContextSnapshot::from_value(
            7,
            2,
            "Fallback",
            "https://fallback.test",
            4,
            100,
            sample_value(),
        )
        .unwrap();

        assert_eq!(snapshot.status, TabContextStatus::Ready);
        assert!(!snapshot.title.contains("user@example.com"));
        assert!(!snapshot.text.contains("topsecret"));
        assert!(!snapshot.text.contains("4111"));
        assert_eq!(snapshot.url, "https://example.com/account");
        assert_eq!(snapshot.elements[1].name, "[REDACTED FIELD]");
        assert_eq!(snapshot.links.len(), 1);
        assert_eq!(snapshot.links[0].url, "https://example.com/docs");
        assert_eq!(snapshot.images.len(), 1);
        assert_eq!(snapshot.images[0].url, "https://example.com/hero.jpg");
        assert_eq!((snapshot.page_width, snapshot.page_height), (1440, 6000));
        assert!(snapshot.estimated_token_count > 0);
        assert_eq!(
            snapshot.view(Some(4), 100).estimated_token_count,
            snapshot.estimated_token_count
        );
        assert!(snapshot.redaction_count >= 7);
        assert!(snapshot.truncated);
    }

    #[test]
    fn estimates_structured_tokens_locally() {
        assert_eq!(estimate_text_tokens(""), 0);
        assert!(estimate_text_tokens("A longer page with several words") > 1);
        assert!(estimate_text_tokens("你好，世界") > 0);

        let short = TabContextSnapshot::from_value(
            1,
            1,
            "Title",
            "https://example.com",
            1,
            10,
            json!({ "text": "Short page" }),
        )
        .unwrap();
        let long = TabContextSnapshot::from_value(
            1,
            2,
            "Title",
            "https://example.com",
            1,
            10,
            json!({ "text": "Longer page content ".repeat(500) }),
        )
        .unwrap();

        assert!(long.estimated_token_count > short.estimated_token_count);
    }

    #[test]
    fn applies_text_and_element_limits() {
        let elements = (0..MAX_CONTEXT_ELEMENTS + 3)
            .map(|index| json!({ "role": "button", "name": format!("Action {index}") }))
            .collect::<Vec<_>>();
        let links = (0..MAX_CONTEXT_LINKS + 3)
            .map(|index| json!({ "href": format!("https://example.com/link/{index}"), "text": format!("Link {index}") }))
            .collect::<Vec<_>>();
        let images = (0..MAX_CONTEXT_IMAGES + 3)
            .map(|index| json!({ "src": format!("https://example.com/image/{index}.jpg"), "alt": format!("Image {index}"), "kind": "image" }))
            .collect::<Vec<_>>();
        let snapshot = TabContextSnapshot::from_value(
            1,
            1,
            "Title",
            "https://example.com",
            1,
            10,
            json!({
                "title": "Title",
                "url": "https://example.com",
                "text": "x".repeat(MAX_CONTEXT_TEXT_CHARS + 20),
                "elements": elements,
                "links": links,
                "images": images,
            }),
        )
        .unwrap();

        assert_eq!(snapshot.text.chars().count(), MAX_CONTEXT_TEXT_CHARS);
        assert_eq!(snapshot.elements.len(), MAX_CONTEXT_ELEMENTS);
        assert_eq!(snapshot.links.len(), MAX_CONTEXT_LINKS);
        assert_eq!(snapshot.images.len(), MAX_CONTEXT_IMAGES);
        assert!(snapshot.truncated);
    }

    #[test]
    fn detects_revision_and_time_staleness() {
        let snapshot = TabContextSnapshot::from_value(
            1,
            1,
            "Title",
            "https://example.com",
            8,
            1_000,
            json!({ "text": "page" }),
        )
        .unwrap();

        assert!(!snapshot.is_stale(Some(8), 2_000));
        assert!(snapshot.is_stale(Some(9), 2_000));
        assert!(snapshot.is_stale(Some(8), 1_000 + CONTEXT_STALE_AFTER_MS + 1));
    }

    #[test]
    fn visual_capture_keeps_snapshot_blocked_until_jpeg_is_ready() {
        let mut snapshot = TabContextSnapshot::from_value(
            1,
            2,
            "Title",
            "https://example.com",
            3,
            100,
            json!({ "text": "page" }),
        )
        .unwrap();
        snapshot.begin_visual_capture();
        assert!(snapshot.is_capturing());

        let encoded = STANDARD.encode([0xff, 0xd8, 0xff, 0x00, 0xff, 0xd9]);
        let jpeg = decode_visual_capture(&json!({ "data": encoded }).to_string()).unwrap();
        snapshot.finish_visual_capture(Some(jpeg), false);

        assert!(snapshot.is_ready());
        assert!(snapshot.visual_jpeg.is_some());
        assert!(snapshot.view(Some(3), 200).visual_included);
    }

    #[test]
    fn visual_capture_builds_a_bounded_sixteen_by_nine_chat_thumbnail() {
        let source = image::RgbImage::from_pixel(640, 1_200, image::Rgb([18, 74, 126]));
        let mut source_jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut source_jpeg, 82)
            .encode_image(&DynamicImage::ImageRgb8(source))
            .unwrap();

        let mut snapshot = TabContextSnapshot::from_value(
            1,
            2,
            "Title",
            "https://example.com",
            3,
            100,
            json!({ "text": "page" }),
        )
        .unwrap();
        snapshot.finish_visual_capture(Some(source_jpeg), false);
        let data_url = snapshot.view(Some(3), 200).preview_data_url.unwrap();
        let encoded = data_url.strip_prefix("data:image/jpeg;base64,").unwrap();
        let thumbnail_bytes = STANDARD.decode(encoded).unwrap();
        let thumbnail =
            image::load_from_memory_with_format(&thumbnail_bytes, ImageFormat::Jpeg).unwrap();

        assert_eq!(
            (thumbnail.width(), thumbnail.height()),
            (VISUAL_THUMBNAIL_WIDTH, VISUAL_THUMBNAIL_HEIGHT)
        );
        assert!(thumbnail_bytes.len() <= MAX_VISUAL_THUMBNAIL_BYTES);
    }

    #[test]
    fn redacts_common_environment_and_url_credentials() {
        let source = concat!(
            "TOKEN=plain-token-value\n",
            "PASSWORD=\"correct horse battery staple\"\n",
            "ACCESS_TOKEN=access-token-value\n",
            "GH_TOKEN=github-token-value\n",
            "CLIENT_SECRET=client-secret-value\n",
            "AWS_SECRET_ACCESS_KEY=aws-secret-value\n",
            "Open http://localhost:8888/?token=jupyter-token-value\n"
        );
        let (redacted, count) = redact_sensitive(source);

        for secret in [
            "plain-token-value",
            "correct horse battery staple",
            "access-token-value",
            "github-token-value",
            "client-secret-value",
            "aws-secret-value",
            "jupyter-token-value",
        ] {
            assert!(!redacted.contains(secret));
        }
        assert!(count >= 6);
    }

    #[test]
    fn safe_stream_prefix_never_splits_a_complete_sensitive_match() {
        let value = "visible user@example.com trailing";
        let inside_email = value.find('@').unwrap();
        assert_eq!(
            redaction_safe_prefix_len(value, inside_email),
            value.find("user@example.com").unwrap()
        );
    }
}
