use std::{
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{sensitive_path::is_sensitive_path, tab_context::sanitize_terminal_snapshot_text};

pub(crate) const MAX_FILE_ATTACHMENTS: usize = 12;
const MAX_TEXT_FILE_BYTES: u64 = 1_000_000;
const MAX_IMAGE_FILE_BYTES: u64 = 8_000_000;
const MAX_AUDIO_FILE_BYTES: u64 = 8_000_000;
const MAX_INLINE_IMAGE_PREVIEW_BYTES: usize = 512_000;
const MAX_FILE_NAME_CHARS: usize = 180;

pub(crate) const AGENT_FILE_DIALOG_EXTENSIONS: &[&str] = &[
    "png",
    "jpg",
    "jpeg",
    "mp3",
    "wav",
    "txt",
    "md",
    "markdown",
    "rs",
    "c",
    "h",
    "cc",
    "cpp",
    "cxx",
    "hpp",
    "cs",
    "go",
    "java",
    "kt",
    "kts",
    "swift",
    "py",
    "rb",
    "php",
    "lua",
    "dart",
    "js",
    "mjs",
    "cjs",
    "jsx",
    "ts",
    "tsx",
    "vue",
    "svelte",
    "astro",
    "html",
    "htm",
    "css",
    "scss",
    "sass",
    "less",
    "json",
    "jsonc",
    "toml",
    "yaml",
    "yml",
    "xml",
    "svg",
    "sql",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "psm1",
    "psd1",
    "bat",
    "cmd",
    "ini",
    "cfg",
    "conf",
    "properties",
    "gradle",
    "lock",
    "gitignore",
    "gitattributes",
    "dockerignore",
    "editorconfig",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileAttachmentKind {
    Text,
    Image,
    Audio,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) enum AgentFileContent {
    Text(String),
    Image(Vec<u8>),
    Audio(Vec<u8>),
}
impl std::fmt::Debug for AgentFileContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(_) => f.write_str("Text(<redacted>)"),
            Self::Image(_) => f.write_str("Image(<redacted>)"),
            Self::Audio(_) => f.write_str("Audio(<redacted>)"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AgentFileInput {
    pub id: String,
    pub name: String,
    pub kind: FileAttachmentKind,
    pub mime_type: String,
    pub byte_count: u64,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    pub content: AgentFileContent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct PendingFileAttachment {
    input: Arc<AgentFileInput>,
    #[serde(skip)]
    native: Arc<OnceLock<NativeParts>>,
    preview_data_url: Option<String>,
    #[serde(default)]
    source_label: Option<String>,
}

struct NativeParts(Vec<serde_json::Value>);
impl std::fmt::Debug for NativeParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeParts(<redacted>)")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileAttachmentView {
    pub id: String,
    #[serde(default)]
    pub icon_key: String,
    pub name: String,
    pub kind: FileAttachmentKind,
    pub mime_type: String,
    pub byte_count: u64,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
}

impl PendingFileAttachment {
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        let link_metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("Could not inspect the selected file: {error}"))?;
        if is_link_like(&link_metadata) {
            return Err("Linked or reparse-point files cannot be attached".to_owned());
        }
        if !link_metadata.is_file() {
            return Err("Only regular files can be attached".to_owned());
        }

        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("Could not open the selected file: {error}"))?;
        if is_sensitive_path(&canonical) {
            return Err("Supervisor does not attach known secret or credential files".to_owned());
        }
        let name = safe_file_name(&canonical)?;
        let classification = classify_file(&name)?;
        let byte_count = link_metadata.len();
        let limit = match classification.kind {
            FileAttachmentKind::Text => MAX_TEXT_FILE_BYTES,
            FileAttachmentKind::Image => MAX_IMAGE_FILE_BYTES,
            FileAttachmentKind::Audio => MAX_AUDIO_FILE_BYTES,
        };
        if byte_count > limit {
            return Err(format!(
                "{name} is too large for a Agent {} attachment (maximum {})",
                match classification.kind {
                    FileAttachmentKind::Text => "text/code",
                    FileAttachmentKind::Image => "image",
                    FileAttachmentKind::Audio => "audio",
                },
                format_bytes(limit)
            ));
        }

        let bytes = fs::read(&canonical)
            .map_err(|error| format!("Could not read the selected file: {error}"))?;
        let (content, estimated_token_count, redaction_count, preview_data_url) =
            match classification.kind {
                FileAttachmentKind::Text => {
                    let raw = std::str::from_utf8(strip_utf8_bom(&bytes)).map_err(|_| {
                        format!("{name} is not valid UTF-8 text and cannot be sent as Agent text")
                    })?;
                    if raw.contains('\0') {
                        return Err(format!(
                            "{name} contains binary data and cannot be attached"
                        ));
                    }
                    let (text, source_chars, redaction_count, _) =
                        sanitize_terminal_snapshot_text(raw);
                    (
                        AgentFileContent::Text(text),
                        source_chars.saturating_add(3) / 4,
                        redaction_count,
                        None,
                    )
                }
                FileAttachmentKind::Image => {
                    validate_image_signature(&bytes, classification.mime_type, &name)?;
                    let preview = (bytes.len() <= MAX_INLINE_IMAGE_PREVIEW_BYTES).then(|| {
                        format!(
                            "data:{};base64,{}",
                            classification.mime_type,
                            BASE64_STANDARD.encode(&bytes)
                        )
                    });
                    (AgentFileContent::Image(bytes), 0, 0, preview)
                }
                FileAttachmentKind::Audio => {
                    validate_audio_signature(&bytes, classification.mime_type, &name)?;
                    (AgentFileContent::Audio(bytes), 0, 0, None)
                }
            };

        let attachment = Self {
            input: Arc::new(AgentFileInput {
                id: Uuid::new_v4().to_string(),
                name,
                kind: classification.kind,
                mime_type: classification.mime_type.to_owned(),
                byte_count,
                estimated_token_count,
                redaction_count,
                content,
            }),
            native: Arc::default(),
            preview_data_url,
            source_label: None,
        };
        // Prepare from the validated, redacted snapshot at attachment time.
        // Cloning a queued/main/graph draft shares bytes and this session-only cache.
        attachment.native_parts();
        Ok(attachment)
    }

    pub(crate) fn id(&self) -> &str {
        &self.input.id
    }

    pub(crate) fn kind(&self) -> FileAttachmentKind {
        self.input.kind
    }

    #[cfg(test)]
    pub(crate) fn input(&self) -> AgentFileInput {
        (*self.input).clone()
    }

    pub(crate) fn content(&self) -> &AgentFileInput {
        &self.input
    }

    pub(crate) fn native_parts(&self) -> &[serde_json::Value] {
        use central_agent_codex_runtime::api;
        &self
            .native
            .get_or_init(|| {
                let file = &self.input;
                let parts = match &file.content {
                    AgentFileContent::Text(text) => vec![api::text_input(&format!(
                        "Attached text file: {}\n\n{text}",
                        file.name
                    ))],
                    AgentFileContent::Image(bytes) | AgentFileContent::Audio(bytes) => {
                        let image = matches!(&file.content, AgentFileContent::Image(_));
                        let label = if image { "image" } else { "audio" };
                        let url = format!(
                            "data:{};base64,{}",
                            file.mime_type,
                            BASE64_STANDARD.encode(bytes)
                        );
                        vec![
                            api::text_input(&format!("Attached {label}: {}", file.name)),
                            if image {
                                api::image_input(&url)
                            } else {
                                api::audio_input(&url)
                            },
                        ]
                    }
                };
                NativeParts(parts)
            })
            .0
    }

    pub(crate) fn view(&self) -> FileAttachmentView {
        FileAttachmentView {
            id: self.input.id.clone(),
            icon_key: crate::time_machine::file_icon_key(&self.input.name).into(),
            name: self.input.name.clone(),
            kind: self.input.kind,
            mime_type: self.input.mime_type.clone(),
            byte_count: self.input.byte_count,
            estimated_token_count: self.input.estimated_token_count,
            redaction_count: self.input.redaction_count,
            preview_data_url: self.preview_data_url.clone(),
            source_label: self.source_label.clone(),
        }
    }
}

struct FileClassification {
    kind: FileAttachmentKind,
    mime_type: &'static str,
}

fn classify_file(name: &str) -> Result<FileClassification, String> {
    let lower = name.to_ascii_lowercase();
    let extension = Path::new(&lower)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    match extension {
        "png" => Ok(FileClassification {
            kind: FileAttachmentKind::Image,
            mime_type: "image/png",
        }),
        "jpg" | "jpeg" => Ok(FileClassification {
            kind: FileAttachmentKind::Image,
            mime_type: "image/jpeg",
        }),
        "mp3" => Ok(FileClassification {
            kind: FileAttachmentKind::Audio,
            mime_type: "audio/mpeg",
        }),
        "wav" => Ok(FileClassification {
            kind: FileAttachmentKind::Audio,
            mime_type: "audio/wav",
        }),
        extension if is_text_extension(extension) || is_special_text_name(&lower) => {
            Ok(FileClassification {
                kind: FileAttachmentKind::Text,
                mime_type: text_mime_type(extension),
            })
        }
        _ => Err(format!(
            "{name} is not a supported Agent input. Attach PNG/JPEG images, MP3/WAV audio, or UTF-8 text/code files"
        )),
    }
}

fn is_text_extension(extension: &str) -> bool {
    AGENT_FILE_DIALOG_EXTENSIONS
        .iter()
        .any(|candidate| *candidate == extension && !matches!(extension, "png" | "jpg" | "jpeg"))
}

fn is_special_text_name(name: &str) -> bool {
    matches!(
        name,
        "dockerfile"
            | "containerfile"
            | "makefile"
            | "gnumakefile"
            | "cmakelists.txt"
            | "justfile"
            | "procfile"
            | "gemfile"
            | "rakefile"
            | ".gitignore"
            | ".gitattributes"
            | ".dockerignore"
            | ".editorconfig"
    ) || name.starts_with("dockerfile.")
        || name.starts_with("containerfile.")
}

fn text_mime_type(extension: &str) -> &'static str {
    match extension {
        "json" | "jsonc" => "application/json",
        "xml" | "svg" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "md" | "markdown" => "text/markdown",
        _ => "text/plain",
    }
}

fn safe_file_name(path: &Path) -> Result<String, String> {
    let raw = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The selected file name is not valid Unicode".to_owned())?;
    let name = raw
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_FILE_NAME_CHARS)
        .collect::<String>();
    if name.is_empty() {
        Err("The selected file has no usable name".to_owned())
    } else {
        Ok(name)
    }
}

fn validate_image_signature(bytes: &[u8], mime_type: &str, name: &str) -> Result<(), String> {
    let valid = match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| format!("{name} does not contain a valid PNG or JPEG image"))
}

fn validate_audio_signature(bytes: &[u8], mime_type: &str, name: &str) -> Result<(), String> {
    let valid = match mime_type {
        "audio/wav" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE",
        "audio/mpeg" => {
            bytes.starts_with(b"ID3")
                || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| format!("{name} does not contain valid MP3 or WAV audio"))
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{} MB", bytes / 1_000_000)
    } else {
        format!("{} KB", bytes / 1_000)
    }
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn prepared_snapshot_is_shared_immutable_and_not_duplicated_in_saved_drafts() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("sample.wav");
        fs::write(&path, b"RIFF\x04\0\0\0WAVEoriginal").unwrap();
        let attachment = PendingFileAttachment::load(&path).unwrap();
        assert!(
            attachment.native.get().is_some(),
            "Encoding must finish at attachment selection"
        );
        let queued = attachment.clone();
        assert!(Arc::ptr_eq(&attachment.input, &queued.input));
        assert!(Arc::ptr_eq(&attachment.native, &queued.native));
        fs::write(&path, b"RIFF\x04\0\0\0WAVEchanged").unwrap();
        assert_eq!(queued.native_parts(), attachment.native_parts());
        let saved = serde_json::to_value(&queued).unwrap();
        assert!(saved.get("native").is_none());
        let restored: PendingFileAttachment = serde_json::from_value(saved).unwrap();
        assert!(restored.native.get().is_none());
        assert_eq!(restored.native_parts(), queued.native_parts());
        assert!(format!("{:?}", attachment.native).contains("redacted"));
    }

    #[test]
    fn loads_utf8_code_as_redacted_text_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("main.rs");
        fs::write(&path, "fn main() { println!(\"ok\"); }\n").unwrap();

        let attachment = PendingFileAttachment::load(&path).unwrap();
        assert_eq!(attachment.kind(), FileAttachmentKind::Text);
        assert!(matches!(
            attachment.input().content,
            AgentFileContent::Text(text) if text.contains("fn main")
        ));
        assert!(!attachment.view().id.is_empty());
    }

    #[test]
    fn accepts_only_real_png_and_jpeg_images() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("reference.png");
        fs::write(&valid, b"\x89PNG\r\n\x1a\nsmall").unwrap();
        assert!(PendingFileAttachment::load(&valid).is_ok());

        let fake = directory.path().join("fake.jpg");
        fs::write(&fake, b"not an image").unwrap();
        assert!(PendingFileAttachment::load(&fake).is_err());
    }

    #[test]
    fn accepts_only_real_mp3_and_wav_audio() {
        let directory = tempdir().unwrap();
        let wav = directory.path().join("sample.wav");
        fs::write(&wav, b"RIFF\x04\0\0\0WAVEdata").unwrap();
        let attachment = PendingFileAttachment::load(&wav).unwrap();
        assert_eq!(attachment.kind(), FileAttachmentKind::Audio);
        assert!(matches!(
            attachment.input().content,
            AgentFileContent::Audio(_)
        ));

        let mp3 = directory.path().join("sample.mp3");
        fs::write(&mp3, b"ID3\x04\0\0fixture").unwrap();
        assert!(PendingFileAttachment::load(&mp3).is_ok());

        let fake = directory.path().join("fake.wav");
        fs::write(&fake, b"not audio").unwrap();
        assert!(PendingFileAttachment::load(&fake).is_err());
    }

    #[test]
    fn rejects_secret_and_binary_files() {
        let directory = tempdir().unwrap();
        let secret = directory.path().join(".env");
        fs::write(&secret, "TOKEN=secret").unwrap();
        assert!(PendingFileAttachment::load(&secret).is_err());

        let binary = directory.path().join("program.exe");
        fs::write(&binary, b"MZ").unwrap();
        assert!(PendingFileAttachment::load(&binary).is_err());
    }

    #[test]
    fn serialized_view_never_exposes_the_source_path_or_file_contents() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Cargo.toml");
        fs::write(&path, "[package]\nname = \"private-example\"\n").unwrap();

        let attachment = PendingFileAttachment::load(&path).unwrap();
        let json = serde_json::to_string(&attachment.view()).unwrap();
        assert!(json.contains("Cargo.toml"));
        assert!(!json.contains(&directory.path().display().to_string()));
        assert!(!json.contains("private-example"));
    }
}
