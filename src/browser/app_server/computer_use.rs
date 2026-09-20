//! Attach verified first-party Computer Use and Browser capabilities to
//! Supervisor's separate Codex App Server profile. No credentials, chat storage
//! or plugin files are copied; only the official shared runtime configuration is
//! mirrored and exact native skill paths are referenced.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use toml_edit::{DocumentMut, Item, value};

const MANAGED: &str = "Supervisor-managed official Computer Use runtime";

pub(super) mod health;
mod retirement;

pub(crate) struct Attachment {
    pub(super) root: Option<PathBuf>,
    pub(super) enabled: bool,
    pub(super) browser_root: Option<PathBuf>,
    pub(super) browser_enabled: bool,
}

pub(super) fn prepare(home: &Path) -> Result<Option<Attachment>, String> {
    #[cfg(not(windows))]
    {
        let _ = home;
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let source = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")));
        let Some(source) = source else {
            return Ok(None);
        };
        let attachment = prepare_from(home, &source)?;
        attachment
            .map(|mut attachment| {
                let config =
                    fs::read_to_string(home.join("config.toml")).map_err(|e| e.to_string())?;
                let config = config.parse::<DocumentMut>().map_err(|e| e.to_string())?;
                attachment.enabled = attachment
                    .root
                    .as_ref()
                    .is_some_and(|root| !host_skill_disabled(&config, root));
                attachment.browser_enabled = attachment.browser_root.as_ref().is_some_and(|root| {
                    !skill_disabled(&config, &root.join("control-in-app-browser/SKILL.md"))
                });
                Ok(attachment)
            })
            .transpose()
    }
}

fn prepare_from(home: &Path, source: &Path) -> Result<Option<Attachment>, String> {
    if !home.is_absolute() || !source.is_absolute() || !home.is_dir() {
        return Err("Computer Use requires separate absolute Codex profiles".into());
    }
    let home = home.canonicalize().map_err(|e| e.to_string())?;
    let source = match source.canonicalize() {
        Ok(path) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => source.to_owned(),
        Err(e) => return Err(e.to_string()),
    };
    if home == source || home.starts_with(&source) || source.starts_with(&home) {
        return Err("Computer Use cannot use overlapping Codex profiles".into());
    }
    retirement::remove_owned(&home)?;
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return Ok(None);
    };
    prepare_from_with_runtime(
        &home,
        &source,
        &PathBuf::from(local)
            .join("OpenAI")
            .join("Codex")
            .join("runtimes"),
    )
}

fn prepare_from_with_runtime(
    home: &Path,
    source: &Path,
    runtime_root: &Path,
) -> Result<Option<Attachment>, String> {
    let browser_pipe = active_supervisor_browser_pipe();
    prepare_from_with_runtime_and_bridge(home, source, runtime_root, browser_pipe.as_deref())
}

fn prepare_from_with_runtime_and_bridge(
    home: &Path,
    source: &Path,
    runtime_root: &Path,
    browser_pipe: Option<&str>,
) -> Result<Option<Attachment>, String> {
    let detected = (|| -> Result<_, String> {
        let source_config = source.join("config.toml");
        let official = if regular_file(&source_config)? {
            let raw = fs::read_to_string(&source_config).map_err(|e| e.to_string())?;
            if raw.len() > 4 * 1024 * 1024 {
                return Err("The official Codex configuration is unexpectedly large".into());
            }
            let doc = raw
                .parse::<DocumentMut>()
                .map_err(|_| "Could not read the official Codex configuration")?;
            let computer_enabled = doc
                .get("plugins")
                .and_then(Item::as_table)
                .and_then(|table| table.get("computer-use@openai-bundled"))
                .and_then(Item::as_table)
                .and_then(|table| table.get("enabled"))
                .and_then(Item::as_bool)
                == Some(true);
            let server = doc
                .get("mcp_servers")
                .and_then(Item::as_table)
                .and_then(|table| table.get("node_repl"))
                .cloned()
                .unwrap_or(Item::None);
            let computer = if computer_enabled && valid_official_server(&server, runtime_root) {
                find_official_skill(source)?.and_then(|(skill, hooks)| {
                    (!host_skill_disabled(&doc, &skill)).then_some((skill, hooks))
                })
            } else {
                None
            };
            let browser =
                if browser_pipe.is_some() && valid_official_runtime_server(&server, runtime_root) {
                    find_official_browser_skill(source, &doc, &server)?
                } else {
                    None
                };
            if computer.is_none() && browser.is_none() {
                None
            } else {
                let hooks = computer
                    .as_ref()
                    .map(|(_, hooks)| hooks.clone())
                    .or_else(|| browser.as_ref().map(|(_, hooks)| hooks.clone()))
                    .ok_or("The official interaction runtime has no hook definition")?;
                Some((
                    server,
                    computer.map(|(root, _)| root),
                    browser.map(|(root, _)| root),
                    hooks,
                ))
            }
        } else {
            None
        };

        Ok(official)
    })();
    let (official, detection_error) = match detected {
        Ok(official) => (official, None),
        Err(error) => (None, Some(error)),
    };

    let destination = home.join("config.toml");
    let raw = if regular_file(&destination)? {
        let raw = fs::read_to_string(&destination).map_err(|e| e.to_string())?;
        if raw.len() > 4 * 1024 * 1024 {
            return Err("Supervisor's Codex configuration is unexpectedly large".into());
        }
        raw
    } else {
        String::new()
    };
    let mut doc = raw
        .parse::<DocumentMut>()
        .map_err(|_| "Could not read Supervisor's separate Codex configuration")?;
    let current = doc
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|table| table.get("node_repl"));
    let managed = current.and_then(Item::as_table).is_some_and(|table| {
        table
            .decor()
            .prefix()
            .and_then(|text| text.as_str())
            .is_some_and(|text| text.contains(MANAGED))
    });
    if current.is_some() && !managed {
        // A user's node_repl configuration is authoritative. Do not attach the
        // Computer Use skill to an unverified server with the same name.
        return Ok(None);
    }
    match official {
        Some((mut server, computer_root, browser_root, hooks)) => {
            if browser_root.is_some() {
                route_browser_to_supervisor(
                    &mut server,
                    browser_pipe.ok_or("Supervisor's browser bridge is unavailable")?,
                )?;
            }
            let Some(table) = server.as_table_mut() else {
                return Ok(None);
            };
            table.decor_mut().set_prefix(format!("# {MANAGED}\n"));
            if doc.get("mcp_servers").is_none() {
                doc["mcp_servers"] = Item::Table(toml_edit::Table::new());
            }
            doc["mcp_servers"]["node_repl"] = server;
            merge_official_hooks(&mut doc, Some(&hooks))?;
            let next = doc.to_string();
            if next != raw {
                atomic_config(&destination, &next)?;
            }
            Ok(Some(Attachment {
                enabled: computer_root.is_some(),
                root: computer_root,
                browser_enabled: browser_root.is_some(),
                browser_root,
            }))
        }
        None => {
            if managed {
                doc["mcp_servers"]
                    .as_table_mut()
                    .ok_or("Invalid managed Computer Use configuration")?
                    .remove("node_repl");
            }
            merge_official_hooks(&mut doc, None)?;
            if doc.to_string() != raw {
                atomic_config(&destination, &doc.to_string())?;
            }
            match detection_error {
                Some(error) => Err(error),
                None => Ok(None),
            }
        }
    }
}

fn active_supervisor_browser_pipe() -> Option<String> {
    #[cfg(windows)]
    {
        crate::browser::browser_use_bridge::pipe_path_if_running()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn route_browser_to_supervisor(server: &mut Item, pipe: &str) -> Result<(), String> {
    if !pipe.starts_with(r"\\.\pipe\codex-browser-use-supervisor-") {
        return Err("Supervisor's browser bridge address is invalid".into());
    }
    let env = server
        .as_table_mut()
        .and_then(|table| table.get_mut("env"))
        .and_then(Item::as_table_mut)
        .ok_or("The official Browser runtime has no environment table")?;
    env["BROWSER_USE_AVAILABLE_BACKENDS"] = value("iab");
    env["CDP_BROWSER_BACKEND_PIPE_PATH"] = value(pipe);
    env["BROWSER_AUTH_EVAL_EXACT_CDP_BACKEND_SOCKET"] = value("true");
    env["BROWSER_USE_CODEX_APP_BUILD_FLAVOR"] = value("supervisor");
    Ok(())
}

fn regular_file(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            let redirected = {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let redirected = false;
            if redirected || meta.file_type().is_symlink() || !meta.is_file() {
                Err("Computer Use configuration cannot follow filesystem links".into())
            } else {
                Ok(true)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn valid_official_server(server: &Item, runtime: &Path) -> bool {
    if !valid_official_runtime_server(server, runtime) {
        return false;
    }
    let Some(env) = server
        .as_table()
        .and_then(|table| table.get("env"))
        .and_then(Item::as_table)
    else {
        return false;
    };
    env.get("SKY_CUA_NATIVE_PIPE").and_then(Item::as_str) == Some("1")
        && env
            .get("SKY_CUA_NATIVE_PIPE_DIRECTORY")
            .and_then(Item::as_str)
            .is_some_and(|value| !value.is_empty())
}

fn valid_official_runtime_server(server: &Item, runtime: &Path) -> bool {
    let Some(table) = server.as_table() else {
        return false;
    };
    if table.get("enabled").and_then(Item::as_bool) == Some(false) {
        return false;
    }
    let Some(command) = table.get("command").and_then(Item::as_str) else {
        return false;
    };
    let executable = Path::new(command);
    if !executable.is_absolute()
        || !executable.is_file()
        || executable
            .file_name()
            .is_none_or(|name| !name.to_string_lossy().eq_ignore_ascii_case("node_repl.exe"))
    {
        return false;
    }
    if !executable.canonicalize().ok().is_some_and(|path| {
        runtime
            .canonicalize()
            .ok()
            .is_some_and(|root| path.starts_with(root))
    }) {
        return false;
    }
    let Some(env) = table.get("env").and_then(Item::as_table) else {
        return false;
    };
    env.get("NODE_REPL_NODE_MODULE_DIRS")
        .and_then(Item::as_str)
        .is_some_and(|value| !value.is_empty())
}

fn host_skill_disabled(doc: &DocumentMut, root: &Path) -> bool {
    let skill = root.join("computer-use").join("SKILL.md");
    skill_disabled(doc, &skill)
}

fn skill_disabled(doc: &DocumentMut, skill: &Path) -> bool {
    let Some(configs) = doc
        .get("skills")
        .and_then(Item::as_table)
        .and_then(|table| table.get("config"))
        .and_then(Item::as_array_of_tables)
    else {
        return false;
    };
    configs.iter().any(|entry| {
        entry.get("enabled").and_then(Item::as_bool) == Some(false)
            && entry
                .get("path")
                .and_then(Item::as_str)
                .is_some_and(|path| {
                    Path::new(path)
                        .canonicalize()
                        .ok()
                        .zip(skill.canonicalize().ok())
                        .is_some_and(|(configured, official)| configured == official)
                })
    })
}

fn find_official_skill(source: &Path) -> Result<Option<(PathBuf, serde_json::Value)>, String> {
    let resolved_source = source.canonicalize().map_err(|e| e.to_string())?;
    let cache = source
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("computer-use");
    if !cache.is_dir() {
        return Ok(None);
    }
    let resolved_cache = cache.canonicalize().map_err(|e| e.to_string())?;
    if !resolved_cache.starts_with(&resolved_source) {
        return Err("The official Computer Use cache is redirected outside Codex".into());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&cache).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let package = entry.path();
        if !package
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&resolved_cache)
        {
            continue;
        }
        let manifest = package.join(".codex-plugin").join("plugin.json");
        let skill = package.join("skills").join("computer-use").join("SKILL.md");
        if !regular_file(&manifest)? || !regular_file(&skill)? {
            continue;
        }
        if fs::metadata(&manifest).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
            continue;
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?)
                .map_err(|_| "The Computer Use plugin manifest is invalid")?;
        if metadata["name"] == "computer-use"
            && metadata["author"]["name"] == "OpenAI"
            && metadata["skills"] == "./skills/"
            && metadata["hooks"]["hooks"].is_object()
        {
            candidates.push((
                entry.file_name(),
                package.join("skills"),
                metadata["hooks"].clone(),
            ));
        }
    }
    if candidates.len() > 1 {
        return Err("Several official Computer Use versions are cached and the active version cannot be verified. Finish the plugin update in Codex desktop, then reconnect Supervisor in AI accounts. No cached version was selected automatically.".into());
    }
    candidates
        .pop()
        .map(|(_, root, hooks)| {
            root.canonicalize()
                .map_err(|e| e.to_string())
                .and_then(|root| {
                    if root.starts_with(&resolved_cache) {
                        Ok((root, hooks))
                    } else {
                        Err("The official Computer Use skill root is redirected".into())
                    }
                })
        })
        .transpose()
}

fn find_official_browser_skill(
    source: &Path,
    config: &DocumentMut,
    server: &Item,
) -> Result<Option<(PathBuf, serde_json::Value)>, String> {
    let enabled = config
        .get("plugins")
        .and_then(Item::as_table)
        .and_then(|table| table.get("browser@openai-bundled"))
        .and_then(Item::as_table)
        .and_then(|table| table.get("enabled"))
        .and_then(Item::as_bool)
        == Some(true);
    if !enabled {
        return Ok(None);
    }
    let resolved_source = source.canonicalize().map_err(|e| e.to_string())?;
    let cache = source
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("browser");
    if !cache.is_dir() {
        return Ok(None);
    }
    let resolved_cache = cache.canonicalize().map_err(|e| e.to_string())?;
    if !resolved_cache.starts_with(&resolved_source) {
        return Err("The official Browser cache is redirected outside Codex".into());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&cache).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let package = entry.path();
        if !package
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&resolved_cache)
        {
            continue;
        }
        let manifest = package.join(".codex-plugin").join("plugin.json");
        let skill = package
            .join("skills")
            .join("control-in-app-browser")
            .join("SKILL.md");
        let client = package.join("scripts").join("browser-client.mjs");
        let service = package.join("scripts").join("browser-service.mjs");
        if !regular_file(&manifest)?
            || !regular_file(&skill)?
            || !regular_file(&client)?
            || !regular_file(&service)?
        {
            continue;
        }
        if fs::metadata(&manifest).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
            continue;
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?)
                .map_err(|_| "The Browser plugin manifest is invalid")?;
        if metadata["name"] == "browser"
            && metadata["author"]["name"] == "OpenAI"
            && metadata["skills"] == "./skills/"
            && metadata["hooks"]["hooks"].is_object()
            && valid_official_browser_server(server, &service)
        {
            candidates.push((
                entry.file_name(),
                package.join("skills"),
                metadata["hooks"].clone(),
            ));
        }
    }
    if candidates.len() > 1 {
        return Err("Several official Browser versions are cached and the active version cannot be verified. Finish the plugin update in Codex desktop, then reconnect Supervisor in AI accounts. No cached version was selected automatically.".into());
    }
    candidates
        .pop()
        .map(|(_, root, hooks)| {
            root.canonicalize()
                .map_err(|e| e.to_string())
                .and_then(|root| {
                    if !root.starts_with(&resolved_cache) {
                        return Err("The official Browser skill root is redirected".into());
                    }
                    if skill_disabled(config, &root.join("control-in-app-browser/SKILL.md")) {
                        Ok(None)
                    } else {
                        Ok(Some((root, hooks)))
                    }
                })
        })
        .transpose()
        .map(Option::flatten)
}

fn valid_official_browser_server(server: &Item, service: &Path) -> bool {
    let Some(env) = server
        .as_table()
        .and_then(|table| table.get("env"))
        .and_then(Item::as_table)
    else {
        return false;
    };
    if !env
        .get("BROWSER_USE_AVAILABLE_BACKENDS")
        .and_then(Item::as_str)
        .is_some_and(|value| value.split(',').any(|backend| backend.trim() == "iab"))
    {
        return false;
    }
    let Some(raw_services) = env.get("NODE_REPL_TRUSTED_SERVICES").and_then(Item::as_str) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(raw_services)
        .ok()
        .and_then(|value| {
            value
                .get("browser")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|path| PathBuf::from(path).canonicalize().ok())
        .zip(service.canonicalize().ok())
        .is_some_and(|(configured, expected)| configured == expected)
}

fn merge_official_hooks(
    doc: &mut DocumentMut,
    hooks: Option<&serde_json::Value>,
) -> Result<(), String> {
    // Remove only entries previously attached by Supervisor. Keep user hooks.
    if let Some(table) = doc.get_mut("hooks").and_then(Item::as_table_mut) {
        for (_, item) in table.iter_mut() {
            if let Some(entries) = item.as_array_of_tables_mut() {
                entries.retain(|entry| {
                    !entry
                        .decor()
                        .prefix()
                        .and_then(|text| text.as_str())
                        .is_some_and(|text| text.contains(MANAGED))
                });
            }
        }
    }
    let Some(hooks) = hooks else {
        return Ok(());
    };
    let source = json_to_item(hooks)?;
    let source_hooks = source
        .as_table()
        .and_then(|table| table.get("hooks"))
        .and_then(Item::as_table)
        .ok_or("The official Computer Use hook definition is incomplete")?;
    if doc.get("hooks").is_none() {
        doc["hooks"] = Item::Table(toml_edit::Table::new());
    }
    let target = doc["hooks"]
        .as_table_mut()
        .ok_or("Supervisor's existing Codex hooks are invalid")?;
    for (event, source_item) in source_hooks.iter() {
        let source_entries = source_item
            .as_array_of_tables()
            .ok_or("The official Computer Use hook event is invalid")?;
        if target.get(event).is_none() {
            target.insert(event, Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
        }
        let destination = target
            .get_mut(event)
            .and_then(Item::as_array_of_tables_mut)
            .ok_or("Supervisor's existing Codex hook event is invalid")?;
        for source_entry in source_entries.iter() {
            let mut entry = source_entry.clone();
            entry.decor_mut().set_prefix(format!("# {MANAGED}\n"));
            destination.push(entry);
        }
    }
    Ok(())
}

fn json_to_item(value: &serde_json::Value) -> Result<Item, String> {
    use serde_json::Value as Json;
    match value {
        Json::Object(map) => {
            let mut table = toml_edit::Table::new();
            for (key, child) in map {
                table.insert(key, json_to_item(child)?);
            }
            Ok(Item::Table(table))
        }
        Json::Array(items) if !items.is_empty() && items.iter().all(Json::is_object) => {
            let mut tables = toml_edit::ArrayOfTables::new();
            for child in items {
                tables.push(
                    json_to_item(child)?
                        .as_table()
                        .ok_or("Invalid Computer Use hook table")?
                        .clone(),
                );
            }
            Ok(Item::ArrayOfTables(tables))
        }
        Json::Array(items) => {
            let mut array = toml_edit::Array::new();
            for child in items {
                let item = json_to_item(child)?;
                array.push(
                    item.as_value()
                        .ok_or("Invalid Computer Use hook array")?
                        .clone(),
                );
            }
            Ok(Item::Value(toml_edit::Value::Array(array)))
        }
        Json::String(value) => Ok(Item::Value(toml_edit::Value::from(value.as_str()))),
        Json::Bool(value) => Ok(Item::Value(toml_edit::Value::from(*value))),
        Json::Number(value) if value.as_i64().is_some() => {
            Ok(Item::Value(toml_edit::Value::from(value.as_i64().unwrap())))
        }
        Json::Number(value) if value.as_f64().is_some() => {
            Ok(Item::Value(toml_edit::Value::from(value.as_f64().unwrap())))
        }
        _ => Err("The official Computer Use hook contains an unsupported value".into()),
    }
}

fn atomic_config(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid Codex configuration path")?;
    let mut stage = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    stage
        .write_all(contents.as_bytes())
        .and_then(|_| stage.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    stage.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_foreign_node_repl_and_separate_profile() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let home = temp.path().join("home");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join("config.toml"),
            "[mcp_servers.node_repl]\ncommand = 'custom'\n",
        )
        .unwrap();
        assert!(prepare_from(&home, &source).unwrap().is_none());
        assert_eq!(
            fs::read_to_string(home.join("config.toml")).unwrap(),
            "[mcp_servers.node_repl]\ncommand = 'custom'\n"
        );
        assert!(prepare_from(&home, &home).is_err());
    }

    #[test]
    fn mirrors_only_official_runtime_and_removes_it_when_disabled() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtimes");
        let executable = runtime.join("bin").join("node_repl.exe");
        let package = source.join("plugins/cache/openai-bundled/computer-use/1.0.0");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::create_dir_all(package.join(".codex-plugin")).unwrap();
        fs::create_dir_all(package.join("skills/computer-use")).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(&executable, "stub").unwrap();
        fs::write(
            package.join(".codex-plugin/plugin.json"),
            r#"{"name":"computer-use","author":{"name":"OpenAI"},"skills":"./skills/","hooks":{"hooks":{"Stop":[{"hooks":[{"type":"mcp_tool","server":"node_repl","tool":"turn_ended","input":{"session_id":"${session_id}","turn_id":"${turn_id}"}}]}]}}}"#,
        )
        .unwrap();
        fs::write(
            package.join("skills/computer-use/SKILL.md"),
            "official skill",
        )
        .unwrap();
        fs::write(home.join("config.toml"), "model = 'test'\n").unwrap();
        let config = format!(
            "[plugins.'computer-use@openai-bundled']\nenabled = true\n[mcp_servers.node_repl]\ncommand = '{}'\n[mcp_servers.node_repl.env]\nSKY_CUA_NATIVE_PIPE = '1'\nSKY_CUA_NATIVE_PIPE_DIRECTORY = 'directory'\nNODE_REPL_NODE_MODULE_DIRS = 'modules'\n",
            executable.display()
        );
        fs::write(source.join("config.toml"), &config).unwrap();
        let attachment = prepare_from_with_runtime(&home, &source, &runtime)
            .unwrap()
            .unwrap();
        assert_eq!(
            attachment.root,
            Some(package.join("skills").canonicalize().unwrap())
        );
        assert!(attachment.browser_root.is_none());
        let first = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(first.contains(MANAGED));
        assert!(first.contains("model = 'test'"));
        assert!(first.contains("[mcp_servers.node_repl]"));
        assert!(first.contains("[[hooks.Stop]]"));
        assert!(first.contains("turn_ended"));
        assert!(
            prepare_from_with_runtime(&home, &source, &runtime)
                .unwrap()
                .is_some()
        );
        assert_eq!(first, fs::read_to_string(home.join("config.toml")).unwrap());
        fs::write(
            source.join("config.toml"),
            config.replace(
                "[mcp_servers.node_repl]\n",
                "[mcp_servers.node_repl]\nenabled = false\n",
            ),
        )
        .unwrap();
        assert!(
            prepare_from_with_runtime(&home, &source, &runtime)
                .unwrap()
                .is_none()
        );
        assert!(
            !fs::read_to_string(home.join("config.toml"))
                .unwrap()
                .contains("node_repl")
        );
        fs::write(
            source.join("config.toml"),
            format!(
                "{config}\n[[skills.config]]\npath = '{}'\nenabled = false\n",
                package.join("skills/computer-use/SKILL.md").display()
            ),
        )
        .unwrap();
        assert!(
            prepare_from_with_runtime(&home, &source, &runtime)
                .unwrap()
                .is_none()
        );
        assert!(
            !fs::read_to_string(home.join("config.toml"))
                .unwrap()
                .contains("node_repl")
        );
        fs::write(
            source.join("config.toml"),
            config.replace("enabled = true", "enabled = false"),
        )
        .unwrap();
        assert!(
            prepare_from_with_runtime(&home, &source, &runtime)
                .unwrap()
                .is_none()
        );
        let last = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(last.contains("model = 'test'"));
        assert!(!last.contains("node_repl"));
        assert!(!last.contains("turn_ended"));
    }

    #[test]
    fn exposes_verified_official_browser_skill_with_the_shared_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtimes");
        let executable = runtime.join("bin/node_repl.exe");
        let computer = source.join("plugins/cache/openai-bundled/computer-use/1");
        let browser = source.join("plugins/cache/openai-bundled/browser/1");
        for package in [&computer, &browser] {
            fs::create_dir_all(package.join(".codex-plugin")).unwrap();
        }
        fs::create_dir_all(computer.join("skills/computer-use")).unwrap();
        fs::create_dir_all(browser.join("skills/control-in-app-browser")).unwrap();
        fs::create_dir_all(browser.join("scripts")).unwrap();
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(&executable, "stub").unwrap();
        fs::write(
            computer.join(".codex-plugin/plugin.json"),
            r#"{"name":"computer-use","author":{"name":"OpenAI"},"skills":"./skills/","hooks":{"hooks":{}}}"#,
        )
        .unwrap();
        fs::write(computer.join("skills/computer-use/SKILL.md"), "computer").unwrap();
        fs::write(
            browser.join(".codex-plugin/plugin.json"),
            r#"{"name":"browser","author":{"name":"OpenAI"},"skills":"./skills/","hooks":{"hooks":{}}}"#,
        )
        .unwrap();
        fs::write(
            browser.join("skills/control-in-app-browser/SKILL.md"),
            "browser",
        )
        .unwrap();
        fs::write(browser.join("scripts/browser-client.mjs"), "client").unwrap();
        let service = browser.join("scripts/browser-service.mjs");
        fs::write(&service, "service").unwrap();
        fs::write(home.join("config.toml"), "model = 'test'\n").unwrap();
        let service_path = service.to_string_lossy().replace('\\', "/");
        fs::write(
            source.join("config.toml"),
            format!(
                "[plugins.'computer-use@openai-bundled']\nenabled = true\n[plugins.'browser@openai-bundled']\nenabled = true\n[mcp_servers.node_repl]\ncommand = '{}'\n[mcp_servers.node_repl.env]\nSKY_CUA_NATIVE_PIPE = '1'\nSKY_CUA_NATIVE_PIPE_DIRECTORY = 'directory'\nNODE_REPL_NODE_MODULE_DIRS = 'modules'\nBROWSER_USE_AVAILABLE_BACKENDS = 'chrome,iab'\nNODE_REPL_TRUSTED_SERVICES = '{{\"browser\":\"{service_path}\"}}'\n",
                executable.display()
            ),
        )
        .unwrap();
        let source_config = fs::read_to_string(source.join("config.toml"))
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        let source_server = source_config["mcp_servers"]["node_repl"].clone();
        assert!(valid_official_browser_server(&source_server, &service));
        assert!(
            find_official_browser_skill(&source, &source_config, &source_server)
                .unwrap()
                .is_some()
        );
        let bridge = r"\\.\pipe\codex-browser-use-supervisor-test";
        let attachment =
            prepare_from_with_runtime_and_bridge(&home, &source, &runtime, Some(bridge))
                .unwrap()
                .unwrap();
        assert_eq!(
            attachment.browser_root,
            Some(browser.join("skills").canonicalize().unwrap())
        );
        assert!(attachment.browser_enabled);
        let routed = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(routed.contains("BROWSER_USE_AVAILABLE_BACKENDS = \"iab\""));
        assert!(routed.contains("BROWSER_AUTH_EVAL_EXACT_CDP_BACKEND_SOCKET = \"true\""));
        assert!(routed.contains("BROWSER_USE_CODEX_APP_BUILD_FLAVOR = \"supervisor\""));
        assert!(routed.contains("codex-browser-use-supervisor-test"));
        fs::write(
            source.join("config.toml"),
            format!(
                "[plugins.'computer-use@openai-bundled']\nenabled = false\n[plugins.'browser@openai-bundled']\nenabled = true\n[mcp_servers.node_repl]\ncommand = '{}'\n[mcp_servers.node_repl.env]\nNODE_REPL_NODE_MODULE_DIRS = 'modules'\nBROWSER_USE_AVAILABLE_BACKENDS = 'iab'\nNODE_REPL_TRUSTED_SERVICES = '{{\"browser\":\"{service_path}\"}}'\n",
                executable.display()
            ),
        )
        .unwrap();
        let browser_only =
            prepare_from_with_runtime_and_bridge(&home, &source, &runtime, Some(bridge))
                .unwrap()
                .unwrap();
        assert!(browser_only.root.is_none());
        assert!(browser_only.browser_root.is_some());
    }

    #[test]
    fn installed_plugin_can_attach_to_disposable_profile_without_screen_actions() {
        let Some(source) = std::env::var_os("SUPERVISOR_TEST_OFFICIAL_CU_SOURCE") else {
            return;
        };
        let disposable = tempfile::tempdir().unwrap();
        let home = disposable.path().join("codex");
        fs::create_dir(&home).unwrap();
        let retired = home.join("skills/supervisor-computer-use/SKILL.md");
        fs::create_dir_all(retired.parent().unwrap()).unwrap();
        fs::write(&retired, "---\nname: supervisor-computer-use\ndescription: Retired fixture\n---\n<!-- Supervisor-managed Computer Use companion -->\n").unwrap();
        let attachment = prepare_from(&home, &PathBuf::from(source))
            .unwrap()
            .unwrap();
        let skill_root = attachment.root.as_ref().unwrap();
        assert!(skill_root.join("computer-use/SKILL.md").is_file());
        assert!(
            !home
                .join("skills/supervisor-computer-use/SKILL.md")
                .exists()
        );
        // Exercise native discovery without creating a thread, calling a model,
        // importing the bridge, reading a screen or submitting desktop input.
        use central_agent_codex_runtime::{
            api,
            runtime::Runtime,
            transport::{Client, Event},
        };
        let runtime = Runtime::discover(disposable.path())
            .unwrap()
            .with_home(&home)
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let client = Client::spawn(&runtime, "computer-use-official-probe", move |event| {
            let _ = tx.send(event);
        })
        .unwrap();
        let outcome = (|| -> Result<(), String> {
            loop {
                match rx
                    .recv_timeout(std::time::Duration::from_secs(30))
                    .map_err(|e| e.to_string())?
                {
                    Event::Ready { .. } => break,
                    Event::Closed { reason } => return Err(reason),
                    Event::ServerRequest { .. } => {
                        return Err("Unexpected native approval request".into());
                    }
                    _ => {}
                }
            }
            api::skills_extra_roots(&[skill_root.to_string_lossy().into_owned()])
                .send(&client)
                .map_err(|e| e.to_string())?
                .wait()
                .map_err(|e| e.to_string())?;
            let listing = api::skills_list(disposable.path().to_str().unwrap())
                .send(&client)
                .map_err(|e| e.to_string())?
                .wait()
                .map_err(|e| e.to_string())?;
            let skills = listing["data"][0]["skills"]
                .as_array()
                .ok_or("Missing native skills")?;
            assert!(skills.iter().any(|entry| {
                entry["enabled"] == true
                    && entry["path"].as_str().is_some_and(|path| {
                        Path::new(path).canonicalize().ok()
                            == skill_root.join("computer-use/SKILL.md").canonicalize().ok()
                    })
            }));
            assert!(!skills.iter().any(|entry| entry["name"] == retirement::NAME));
            Ok(())
        })();
        client.shutdown();
        outcome.unwrap();
        let config = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(config.contains(MANAGED));
        assert!(config.contains("[[hooks.Stop]]"));
        assert!(config.contains("turn_ended"));
    }

    #[test]
    fn missing_official_profile_removes_managed_runtime_only() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        fs::create_dir(&home).unwrap();
        fs::write(
            home.join("config.toml"),
            format!("model = 'keep'\n# {MANAGED}\n[mcp_servers.node_repl]\ncommand = 'obsolete'\n"),
        )
        .unwrap();
        assert!(
            prepare_from(&home, &temp.path().join("missing"))
                .unwrap()
                .is_none()
        );
        let config = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(config.contains("model = 'keep'"));
        assert!(!config.contains("node_repl"));
    }

    #[test]
    fn multiple_cached_versions_are_not_selected_by_directory_sort_order() {
        let temp = tempfile::tempdir().unwrap();
        for version in ["9.0.0", "10.0.0"] {
            let package = temp
                .path()
                .join("plugins/cache/openai-bundled/computer-use")
                .join(version);
            fs::create_dir_all(package.join(".codex-plugin")).unwrap();
            fs::create_dir_all(package.join("skills/computer-use")).unwrap();
            fs::write(package.join(".codex-plugin/plugin.json"),
                r#"{"name":"computer-use","author":{"name":"OpenAI"},"skills":"./skills/","hooks":{"hooks":{}}}"#).unwrap();
            fs::write(
                package.join("skills/computer-use/SKILL.md"),
                "official fixture",
            )
            .unwrap();
        }
        assert!(
            find_official_skill(temp.path())
                .unwrap_err()
                .contains("active version cannot be verified")
        );
    }

    #[test]
    fn reconnect_updates_pipe_and_preserves_unrelated_configuration() {
        // The full mirroring test establishes authority; this verifies that a
        // new source address replaces the managed address without replaying work.
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtimes");
        let executable = runtime.join("node_repl.exe");
        let package = source.join("plugins/cache/openai-bundled/computer-use/1");
        fs::create_dir_all(&runtime).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(package.join(".codex-plugin")).unwrap();
        fs::create_dir_all(package.join("skills/computer-use")).unwrap();
        fs::write(&executable, "fixture").unwrap();
        fs::write(package.join(".codex-plugin/plugin.json"), r#"{"name":"computer-use","author":{"name":"OpenAI"},"skills":"./skills/","hooks":{"hooks":{}}}"#).unwrap();
        fs::write(package.join("skills/computer-use/SKILL.md"), "fixture").unwrap();
        fs::write(home.join("config.toml"), "model = 'keep-model'\n").unwrap();
        for address in ["old-address", "new-address"] {
            fs::write(source.join("config.toml"), format!("[plugins.'computer-use@openai-bundled']\nenabled = true\n[mcp_servers.node_repl]\ncommand = '{}'\n[mcp_servers.node_repl.env]\nSKY_CUA_NATIVE_PIPE = '1'\nSKY_CUA_NATIVE_PIPE_DIRECTORY = '{address}'\nNODE_REPL_NODE_MODULE_DIRS = 'modules'\n", executable.display())).unwrap();
            assert!(
                prepare_from_with_runtime(&home, &source, &runtime)
                    .unwrap()
                    .is_some()
            );
            let actual = fs::read_to_string(home.join("config.toml")).unwrap();
            assert!(actual.contains(address));
            assert!(actual.contains("keep-model"));
            if address == "new-address" {
                assert!(!actual.contains("old-address"));
            }
        }
        fs::write(source.join("config.toml"), "malformed = [").unwrap();
        assert!(prepare_from_with_runtime(&home, &source, &runtime).is_err());
        let preserved = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(preserved.contains("keep-model"));
        assert!(!preserved.contains("node_repl"));
    }
}
