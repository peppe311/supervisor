use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

use anyhow::{Context, anyhow};
use tracing::warn;

pub(crate) const UI_DEVELOPMENT_ROOT_ENV: &str = "CENTRAL_AGENT_UI_DEV_ROOT";

pub(crate) const WATCHED_UI_FILES: &[&str] = &[
    "assets/agent-panel.html",
    "assets/backdrop.html",
    "assets/supervisor-mark.svg",
    "assets/supervisor-wordmark.svg",
    "assets/file-icons.js",
    "assets/fonts/inter/Inter-Variable.woff2",
    "assets/agent-graph.html",
    "assets/preview.html",
    "assets/remote-desktop.html",
    "assets/start-page.html",
    "assets/themes.css",
    "assets/toolbar.html",
    "ui/dist/central-agent-ui.css",
    "ui/dist/central-agent-ui.js",
];

const WATCH_INTERVAL: Duration = Duration::from_millis(180);
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFingerprint {
    bytes: u64,
    modified_nanos: u128,
}

pub(crate) fn configured_root() -> anyhow::Result<Option<PathBuf>> {
    let Some(configured) = std::env::var_os(UI_DEVELOPMENT_ROOT_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return Ok(None);
    };

    if !cfg!(debug_assertions) {
        warn!(
            variable = UI_DEVELOPMENT_ROOT_ENV,
            "external UI development assets are disabled in release builds"
        );
        return Ok(None);
    }

    let root = configured.canonicalize().with_context(|| {
        format!(
            "{} does not select an accessible project directory: {}",
            UI_DEVELOPMENT_ROOT_ENV,
            configured.display()
        )
    })?;
    if !root.join("Cargo.toml").is_file() {
        return Err(anyhow!(
            "{} must select the Supervisor repository root (Cargo.toml was not found)",
            UI_DEVELOPMENT_ROOT_ENV
        ));
    }
    for relative in WATCHED_UI_FILES {
        let path = checked_asset_path(&root, relative).ok_or_else(|| {
            anyhow!("the development UI allow-list contains an invalid path: {relative}")
        })?;
        if !path.is_file() {
            return Err(anyhow!(
                "the development UI asset is missing: {}",
                path.display()
            ));
        }
    }
    Ok(Some(root))
}

pub(crate) fn read_text(development_root: Option<&Path>, relative: &str, embedded: &str) -> String {
    let Some(root) = development_root else {
        return embedded.to_owned();
    };
    let Some(path) = checked_asset_path(root, relative) else {
        warn!(%relative, "invalid development UI asset path was rejected");
        return embedded.to_owned();
    };
    match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            warn!(%error, path = %path.display(), "development UI asset could not be read; using the embedded copy");
            embedded.to_owned()
        }
    }
}

pub(crate) fn read_bytes<'a>(
    development_root: Option<&Path>,
    relative: &str,
    embedded: &'a [u8],
) -> std::borrow::Cow<'a, [u8]> {
    let Some(root) = development_root else {
        return embedded.into();
    };
    let Some(path) = checked_asset_path(root, relative) else {
        warn!(%relative, "invalid development UI asset path was rejected");
        return embedded.into();
    };
    match fs::read(&path) {
        Ok(bytes) => bytes.into(),
        Err(error) => {
            warn!(%error, path = %path.display(), "development UI asset could not be read; using the embedded copy");
            embedded.into()
        }
    }
}

pub(crate) fn spawn_watcher<F>(root: PathBuf, mut publish: F)
where
    F: FnMut(Vec<String>) -> bool + Send + 'static,
{
    thread::spawn(move || {
        let mut previous = loop {
            if let Some(snapshot) = snapshot(&root) {
                break snapshot;
            }
            thread::sleep(WATCH_INTERVAL);
        };
        let mut pending = BTreeSet::new();
        let mut last_change = None;

        loop {
            thread::sleep(WATCH_INTERVAL);
            let Some(current) = snapshot(&root) else {
                continue;
            };
            for relative in WATCHED_UI_FILES {
                if previous.get(*relative) != current.get(*relative) {
                    pending.insert((*relative).to_owned());
                    last_change = Some(Instant::now());
                }
            }
            previous = current;

            if pending.is_empty()
                || last_change.is_some_and(|changed_at| changed_at.elapsed() < RELOAD_DEBOUNCE)
            {
                continue;
            }
            let changed = pending.iter().cloned().collect::<Vec<_>>();
            pending.clear();
            last_change = None;
            if !publish(changed) {
                break;
            }
        }
    });
}

fn checked_asset_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(root.join(relative))
}

fn snapshot(root: &Path) -> Option<BTreeMap<&'static str, FileFingerprint>> {
    let mut snapshot = BTreeMap::new();
    for relative in WATCHED_UI_FILES {
        let path = checked_asset_path(root, relative)?;
        let metadata = fs::metadata(path).ok()?;
        let modified_nanos = metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        snapshot.insert(
            *relative,
            FileFingerprint {
                bytes: metadata.len(),
                modified_nanos,
            },
        );
    }
    Some(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_asset_reads_only_relative_paths_and_falls_back_safely() {
        let directory = tempfile::tempdir().unwrap();
        let asset = directory.path().join("assets").join("surface.html");
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&asset, "development").unwrap();

        assert_eq!(
            read_text(Some(directory.path()), "assets/surface.html", "embedded"),
            "development"
        );
        assert_eq!(
            read_text(Some(directory.path()), "../outside.html", "embedded"),
            "embedded"
        );
        assert_eq!(
            read_text(Some(directory.path()), "assets/missing.html", "embedded"),
            "embedded"
        );
    }

    #[test]
    fn development_binary_assets_preserve_bytes_and_fall_back_safely() {
        let directory = tempfile::tempdir().unwrap();
        let bytes = [0, 1, 255, 128];
        fs::write(directory.path().join("font.ttf"), bytes).unwrap();
        assert_eq!(
            read_bytes(Some(directory.path()), "font.ttf", b"fallback").as_ref(),
            bytes
        );
        for path in ["../font.ttf", "missing.ttf"] {
            assert_eq!(
                read_bytes(Some(directory.path()), path, b"fallback").as_ref(),
                b"fallback"
            );
        }
        assert_eq!(read_bytes(None, "font.ttf", &bytes).as_ref(), bytes);
    }
}
