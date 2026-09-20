use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::{fs::OpenOptions, io::Write};
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
const MAX_STORED_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IMAGE_PREVIEW_BYTES: u64 = 12 * 1024 * 1024;
const MAX_AUDIO_PREVIEW_BYTES: u64 = 24 * 1024 * 1024;
const MAX_VIDEO_PREVIEW_BYTES: u64 = 48 * 1024 * 1024;
const MAX_DOCUMENT_PREVIEW_BYTES: u64 = 18 * 1024 * 1024;
const MAX_TEXT_PREVIEW_BYTES: usize = 512 * 1024;
const MAX_INTERACTIVE_PREVIEW_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChatArtifactKind {
    Image,
    File,
    Code,
    Diff,
    Diagram,
    Audio,
    Video,
    InteractivePreview,
    Citation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatArtifact {
    pub id: String,
    pub kind: ChatArtifactKind,
    pub title: String,
    pub display_path: Option<String>,
    pub mime_type: Option<String>,
    pub byte_count: Option<u64>,
    pub storage_key: Option<String>,
    pub language: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ArtifactPreview {
    data_url: Option<String>,
    text: Option<String>,
    truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatArtifactView<'a> {
    pub id: &'a str,
    pub kind: ChatArtifactKind,
    pub title: &'a str,
    pub display_path: Option<&'a str>,
    pub mime_type: Option<&'a str>,
    pub byte_count: Option<u64>,
    pub language: Option<&'a str>,
    pub source_url: Option<&'a str>,
    pub preview_data_url: Option<String>,
    pub preview_text: Option<String>,
    pub preview_truncated: bool,
    pub can_open: bool,
    pub can_save: bool,
    pub can_show_in_folder: bool,
}

pub(crate) struct ArtifactStore {
    files: PathBuf,
    cache: Mutex<HashMap<String, ArtifactPreview>>,
}

impl ArtifactStore {
    pub(crate) fn open(root: PathBuf) -> Result<Self, String> {
        let files = root.join("files");
        fs::create_dir_all(&files).map_err(|error| {
            format!(
                "Artifact storage could not be prepared at {}: {error}",
                root.display()
            )
        })?;
        Ok(Self {
            files,
            cache: Mutex::new(HashMap::new()),
        })
    }

    #[cfg(test)]
    fn ingest_bytes(
        &self,
        bytes: &[u8],
        file_name: &str,
        display_path: impl Into<String>,
    ) -> Result<ChatArtifact, String> {
        if bytes.len() as u64 > MAX_STORED_ARTIFACT_BYTES {
            return Err(format!(
                "Artifact output is larger than {} MiB",
                MAX_STORED_ARTIFACT_BYTES / 1024 / 1024
            ));
        }
        let title = sanitize_file_name(file_name);
        let title = if title.is_empty() {
            "Generated artifact".to_owned()
        } else {
            title
        };
        let extension = Path::new(&title)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .unwrap_or_default();
        let content_hash = format!("{:x}", Sha256::digest(bytes));
        let storage_key = if extension.is_empty() {
            content_hash.clone()
        } else {
            format!("{content_hash}.{extension}")
        };
        let stored = self.files.join(&storage_key);
        if !stored.exists() {
            let temp = self.files.join(format!(".ingest-{}.tmp", Uuid::new_v4()));
            let write_result = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .and_then(|mut file| {
                    file.write_all(bytes)?;
                    file.flush()
                });
            if let Err(error) = write_result {
                let _ = fs::remove_file(&temp);
                return Err(format!("Artifact copy could not be written: {error}"));
            }
            if let Err(error) = fs::rename(&temp, &stored) {
                let _ = fs::remove_file(&temp);
                return Err(format!("Artifact copy could not be stored: {error}"));
            }
        }

        let display_path = normalize_display_path(&display_path.into());
        let id_hash = Sha256::digest(format!("{content_hash}:{display_path}").as_bytes());
        let id_hash = format!("{id_hash:x}");
        Ok(ChatArtifact {
            id: format!("artifact-{}", &id_hash[..24]),
            kind: artifact_kind_for_extension(&extension),
            title,
            display_path: (!display_path.is_empty()).then_some(display_path),
            mime_type: mime_type_for_extension(&extension).map(str::to_owned),
            byte_count: Some(bytes.len() as u64),
            storage_key: Some(storage_key),
            language: language_for_extension(&extension).map(str::to_owned),
            source_url: None,
        })
    }

    pub(crate) fn view<'a>(&self, artifact: &'a ChatArtifact) -> ChatArtifactView<'a> {
        let preview = self.preview(artifact).unwrap_or_default();
        let has_file = artifact.storage_key.is_some();
        ChatArtifactView {
            id: &artifact.id,
            kind: artifact.kind,
            title: &artifact.title,
            display_path: artifact.display_path.as_deref(),
            mime_type: artifact.mime_type.as_deref(),
            byte_count: artifact.byte_count,
            language: artifact.language.as_deref(),
            source_url: artifact.source_url.as_deref(),
            preview_data_url: preview.data_url,
            preview_text: preview.text,
            preview_truncated: preview.truncated,
            can_open: has_file || artifact.source_url.is_some(),
            can_save: has_file,
            can_show_in_folder: has_file,
        }
    }

    pub(crate) fn stored_path(&self, artifact: &ChatArtifact) -> Result<PathBuf, String> {
        let key = artifact
            .storage_key
            .as_deref()
            .ok_or_else(|| "This artifact does not contain a local file".to_owned())?;
        validate_storage_key(key)?;
        let path = self.files.join(key);
        if !path.is_file() {
            return Err("The stored artifact file is no longer available".to_owned());
        }
        Ok(path)
    }

    pub(crate) fn save_as(
        &self,
        artifact: &ChatArtifact,
        destination: &Path,
    ) -> Result<(), String> {
        let source = self.stored_path(artifact)?;
        fs::copy(source, destination)
            .map(|_| ())
            .map_err(|error| format!("Artifact could not be saved: {error}"))
    }

    fn preview(&self, artifact: &ChatArtifact) -> Result<ArtifactPreview, String> {
        if let Ok(cache) = self.cache.lock()
            && let Some(preview) = cache.get(&artifact.id)
        {
            return Ok(preview.clone());
        }
        let preview = self.load_preview(artifact)?;
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(artifact.id.clone(), preview.clone());
        }
        Ok(preview)
    }

    fn load_preview(&self, artifact: &ChatArtifact) -> Result<ArtifactPreview, String> {
        let Some(_) = artifact.storage_key else {
            return Ok(ArtifactPreview::default());
        };
        let path = self.stored_path(artifact)?;
        let byte_count = artifact.byte_count.unwrap_or_default();
        let mime = artifact
            .mime_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let data_limit = match artifact.kind {
            ChatArtifactKind::Image => Some(MAX_IMAGE_PREVIEW_BYTES),
            ChatArtifactKind::Audio => Some(MAX_AUDIO_PREVIEW_BYTES),
            ChatArtifactKind::Video => Some(MAX_VIDEO_PREVIEW_BYTES),
            ChatArtifactKind::File if mime == "application/pdf" => Some(MAX_DOCUMENT_PREVIEW_BYTES),
            _ => None,
        };
        if let Some(limit) = data_limit {
            if byte_count > limit {
                return Ok(ArtifactPreview {
                    truncated: true,
                    ..ArtifactPreview::default()
                });
            }
            let bytes = fs::read(path)
                .map_err(|error| format!("Artifact preview could not be read: {error}"))?;
            return Ok(ArtifactPreview {
                data_url: Some(format!(
                    "data:{mime};base64,{}",
                    BASE64_STANDARD.encode(bytes)
                )),
                text: None,
                truncated: false,
            });
        }
        if matches!(
            artifact.kind,
            ChatArtifactKind::Code
                | ChatArtifactKind::Diff
                | ChatArtifactKind::Diagram
                | ChatArtifactKind::InteractivePreview
        ) || mime.starts_with("text/")
        {
            let limit = if artifact.kind == ChatArtifactKind::InteractivePreview {
                MAX_INTERACTIVE_PREVIEW_BYTES
            } else {
                MAX_TEXT_PREVIEW_BYTES
            };
            let (text, truncated) = read_text_prefix(&path, limit)?;
            return Ok(ArtifactPreview {
                data_url: None,
                text: Some(text),
                truncated,
            });
        }
        Ok(ArtifactPreview::default())
    }
}

pub(crate) fn open_with_default(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(path);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Artifact could not be opened: {error}"))
}

pub(crate) fn show_in_folder(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(format!("/select,{}", path.display()));
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or(path));
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Artifact folder could not be opened: {error}"))
}

pub(crate) fn extract_citation_artifacts(markdown: &str) -> Vec<ChatArtifact> {
    let Ok(link) = Regex::new(r#"\[([^\]]+)\]\((https?://[^\s\)]+)(?:\s+\"[^\"]*\")?\)"#) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    link.captures_iter(markdown)
        .filter_map(|capture| {
            let url = capture.get(2)?.as_str().trim().to_owned();
            if !seen.insert(url.clone()) {
                return None;
            }
            let title = capture
                .get(1)
                .map(|value| value.as_str().trim())
                .filter(|value| !value.is_empty())
                .unwrap_or("Source")
                .to_owned();
            let digest = Sha256::digest(url.as_bytes());
            let digest = format!("{digest:x}");
            Some(ChatArtifact {
                id: format!("citation-{}", &digest[..24]),
                kind: ChatArtifactKind::Citation,
                title,
                display_path: None,
                mime_type: None,
                byte_count: None,
                storage_key: None,
                language: None,
                source_url: Some(url),
            })
        })
        .collect()
}

#[cfg(test)]
fn artifact_kind_for_extension(extension: &str) -> ChatArtifactKind {
    match extension {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => ChatArtifactKind::Image,
        "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => ChatArtifactKind::Audio,
        "mp4" | "webm" | "mov" | "mkv" | "avi" => ChatArtifactKind::Video,
        "html" | "htm" => ChatArtifactKind::InteractivePreview,
        "mmd" | "mermaid" | "dot" | "svg" => ChatArtifactKind::Diagram,
        "diff" | "patch" => ChatArtifactKind::Diff,
        "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" | "ts" | "tsx" | "js"
        | "jsx" | "mjs" | "cjs" | "py" | "pyi" | "go" | "java" | "kt" | "kts" | "cs" | "swift"
        | "rb" | "php" | "css" | "scss" | "sass" | "less" | "json" | "jsonc" | "toml" | "yaml"
        | "yml" | "md" | "mdx" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "psm1" | "psd1"
        | "sql" | "xml" | "txt" => ChatArtifactKind::Code,
        _ => ChatArtifactKind::File,
    }
}

#[cfg(test)]
fn mime_type_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "pdf" => "application/pdf",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "cjs" => "text/javascript",
        "json" | "jsonc" => "application/json",
        "md" | "mdx" => "text/markdown",
        "csv" => "text/csv",
        "txt" | "mmd" | "mermaid" | "dot" | "diff" | "patch" => "text/plain",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        _ if language_for_extension(extension).is_some() => "text/plain",
        _ => return None,
    })
}

#[cfg(test)]
fn language_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "rs" => "rust",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "cs" => "csharp",
        "swift" => "swift",
        "rb" => "ruby",
        "php" => "php",
        "html" | "htm" => "html",
        "css" | "scss" | "sass" | "less" => "css",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "mdx" => "markdown",
        "sh" | "bash" | "zsh" | "fish" => "shell",
        "ps1" | "psm1" | "psd1" => "powershell",
        "sql" => "sql",
        "xml" | "svg" => "xml",
        "mmd" | "mermaid" => "mermaid",
        "dot" => "dot",
        "diff" | "patch" => "diff",
        "txt" => "text",
        _ => return None,
    })
}

fn read_text_prefix(path: &Path, limit: usize) -> Result<(String, bool), String> {
    let file = File::open(path)
        .map_err(|error| format!("Artifact preview could not be opened: {error}"))?;
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    file.take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Artifact preview could not be read: {error}"))?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    let text = String::from_utf8(bytes)
        .map_err(|_| "Artifact text preview is not valid UTF-8".to_owned())?;
    Ok((text, truncated))
}

fn validate_storage_key(key: &str) -> Result<(), String> {
    let path = Path::new(key);
    if key.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component
                    .as_os_str()
                    .to_string_lossy()
                    .contains(['/', '\\'])
        })
    {
        return Err("Artifact storage reference is invalid".to_owned());
    }
    Ok(())
}

pub(crate) fn normalize_display_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        .take(160)
        .collect::<String>()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_owned()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn ingests_and_previews_generated_artifacts_without_exposing_source_paths() {
        let temp = TempDir::new().unwrap();
        let store = ArtifactStore::open(temp.path().join("artifacts")).unwrap();
        let artifact = store
            .ingest_bytes(
                b"fn main() { println!(\"ready\"); }\n",
                "result.rs",
                "src/result.rs",
            )
            .unwrap();
        let view = store.view(&artifact);

        assert_eq!(artifact.kind, ChatArtifactKind::Code);
        assert_eq!(artifact.language.as_deref(), Some("rust"));
        assert_eq!(view.display_path, Some("src/result.rs"));
        assert!(
            view.preview_text
                .as_deref()
                .is_some_and(|preview| preview.contains("println!"))
        );
        assert!(
            !serde_json::to_string(&view)
                .unwrap()
                .contains(temp.path().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn image_preview_is_a_bounded_data_url() {
        let temp = TempDir::new().unwrap();
        let store = ArtifactStore::open(temp.path().join("artifacts")).unwrap();
        let artifact = store
            .ingest_bytes(&[0x89, b'P', b'N', b'G'], "frame.png", "frame.png")
            .unwrap();
        let view = store.view(&artifact);

        assert_eq!(artifact.kind, ChatArtifactKind::Image);
        assert!(
            view.preview_data_url
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }

    #[test]
    fn ingests_provider_bytes_before_the_provider_temp_file_is_removed() {
        let temp = TempDir::new().unwrap();
        let store = ArtifactStore::open(temp.path().join("artifacts")).unwrap();
        let artifact = store
            .ingest_bytes(
                &[0x89, b'P', b'N', b'G'],
                "generated.png",
                "Agent output/generated.png",
            )
            .unwrap();

        assert_eq!(artifact.kind, ChatArtifactKind::Image);
        assert_eq!(artifact.title, "generated.png");
        assert!(store.stored_path(&artifact).unwrap().is_file());
    }

    #[test]
    fn extracts_deduplicated_markdown_citations() {
        let citations = extract_citation_artifacts(
            "See [OpenAI](https://developers.openai.com/docs) and [again](https://developers.openai.com/docs).",
        );
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].kind, ChatArtifactKind::Citation);
        assert_eq!(citations[0].title, "OpenAI");
    }
}
