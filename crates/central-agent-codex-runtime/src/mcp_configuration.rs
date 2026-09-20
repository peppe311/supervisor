//! Versioned MCP configuration intents. Codex owns TOML, layering, validation
//! and process startup. No direct file writes or secret-value projection.
use crate::{
    api::{self, Call},
    configuration::{Snapshot as Preferences, Target},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub name: String,
    pub transport: &'static str,
    pub effective_enabled: Option<bool>,
    pub in_user_config: bool,
    pub can_edit: bool,
    pub user_transport: &'static str,
    pub saved_options: Vec<OptionKey>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub target: Option<Target>,
    pub servers: Vec<Server>,
    #[serde(skip)]
    observed: BTreeSet<String>,
}

/// Deliberately excludes inline credentials, raw environment values, arbitrary
/// headers and native config paths/revisions. These are not a generic RPC editor.
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Add {
        name: String,
        transport: Transport,
    },
    SetEnabled {
        name: String,
        enabled: bool,
    },
    Remove {
        name: String,
    },
    SetOption {
        name: String,
        key: OptionKey,
        value: Value,
    },
    ClearOption {
        name: String,
        key: OptionKey,
    },
}

/// Public connection/behavior keys only. Existing values (which may contain
/// credentials) are never projected; edits carry only newly typed values.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptionKey {
    Command,
    Args,
    Cwd,
    EnvVars,
    Url,
    BearerTokenEnvVar,
    EnvHttpHeaders,
    StartupTimeoutSec,
    ToolTimeoutSec,
    Required,
    EnabledTools,
    DisabledTools,
}
impl OptionKey {
    const ALL: [Self; 12] = [
        Self::Command,
        Self::Args,
        Self::Cwd,
        Self::EnvVars,
        Self::Url,
        Self::BearerTokenEnvVar,
        Self::EnvHttpHeaders,
        Self::StartupTimeoutSec,
        Self::ToolTimeoutSec,
        Self::Required,
        Self::EnabledTools,
        Self::DisabledTools,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Args => "args",
            Self::Cwd => "cwd",
            Self::EnvVars => "env_vars",
            Self::Url => "url",
            Self::BearerTokenEnvVar => "bearer_token_env_var",
            Self::EnvHttpHeaders => "env_http_headers",
            Self::StartupTimeoutSec => "startup_timeout_sec",
            Self::ToolTimeoutSec => "tool_timeout_sec",
            Self::Required => "required",
            Self::EnabledTools => "enabled_tools",
            Self::DisabledTools => "disabled_tools",
        }
    }
    fn supports(self, transport: &str) -> bool {
        match self {
            Self::Command | Self::Args | Self::Cwd | Self::EnvVars => transport == "stdio",
            Self::Url | Self::BearerTokenEnvVar | Self::EnvHttpHeaders => transport == "http",
            _ => matches!(transport, "stdio" | "http"),
        }
    }
    fn clearable(self) -> bool {
        !matches!(self, Self::Command | Self::Url)
    }
    fn validate(self, value: &Value) -> Result<(), String> {
        let valid = match self {
            Self::Command => value
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && !s.chars().any(char::is_control)),
            Self::Cwd => value
                .as_str()
                .is_some_and(|s| !s.contains('\0') && Path::new(s).is_absolute()),
            Self::BearerTokenEnvVar => value.as_str().is_some_and(env_name),
            Self::Url => value.as_str().is_some_and(|url| {
                Transport::Http {
                    url: url.into(),
                    bearer_token_env_var: None,
                }
                .configuration()
                .is_ok()
            }),
            Self::Args | Self::EnvVars | Self::EnabledTools | Self::DisabledTools => {
                value.as_array().is_some_and(|a| {
                    a.iter().all(|v| {
                        v.as_str().is_some_and(|s| {
                            !s.contains('\0')
                                && if self == Self::EnvVars {
                                    env_name(s)
                                } else if self == Self::Args {
                                    true
                                } else {
                                    !s.trim().is_empty()
                                }
                        })
                    })
                })
            }
            Self::EnvHttpHeaders => value.as_object().is_some_and(|map| {
                map.iter().all(|(key, value)| {
                    !key.is_empty()
                        && key
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
                        && value.as_str().is_some_and(env_name)
                })
            }),
            Self::StartupTimeoutSec | Self::ToolTimeoutSec => {
                value.as_f64().is_some_and(|n| n.is_finite() && n > 0.0)
            }
            Self::Required => value.is_boolean(),
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "Invalid {} value. Use the typed field; clearing a saved option is a separate action.",
                self.name()
            ))
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    Stdio {
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env_vars: Vec<String>,
    },
    Http {
        url: String,
        bearer_token_env_var: Option<String>,
    },
}
impl std::fmt::Debug for Edit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeMcpConfigurationEdit(<redacted>)")
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn env_name(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn servers(value: &Value) -> Result<Option<&serde_json::Map<String, Value>>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_object()
        .ok_or("Malformed native MCP server configuration")?;
    if values.values().any(|server| !server.is_object()) {
        return Err("Malformed native MCP server entry".into());
    }
    Ok(Some(values))
}
fn reported_enabled(server: Option<&Value>) -> Result<Option<bool>, String> {
    let Some(value) = server.and_then(|server| server.get("enabled")) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| "Malformed native MCP enabled state".into())
}

fn transport_of(server: Option<&Value>) -> &'static str {
    match server {
        Some(v) if v["command"].is_string() && !v["url"].is_string() => "stdio",
        Some(v) if v["url"].is_string() && !v["command"].is_string() => "http",
        _ => "unknown",
    }
}

impl Snapshot {
    pub fn read(value: &Value) -> Result<Self, String> {
        // Reuse the existing strict native base-user layer/path/version resolver.
        let preferences = Preferences::read(value)?;
        let effective = servers(&value["config"]["mcp_servers"])?;
        let mut observed = BTreeSet::new();
        let mut user = None;
        for layer in value["layers"].as_array().ok_or("Missing native layers")? {
            let entries = servers(&layer["config"]["mcp_servers"])?;
            if let Some(entries) = entries {
                observed.extend(entries.keys().cloned());
            }
            if preferences.target.as_ref().is_some_and(|target| {
                layer["name"]["type"] == "user"
                    && layer["name"]["profile"].is_null()
                    && layer["name"]["file"] == target.file
            }) {
                user = entries;
            }
        }
        if let Some(effective) = effective {
            observed.extend(effective.keys().cloned());
        }
        let listed = observed
            .iter()
            .map(|name| {
                let native = effective.and_then(|values| values.get(name));
                let stored = user.and_then(|values| values.get(name));
                let description = native.or(stored);
                let transport = match description {
                    Some(server) if server["command"].is_string() && !server["url"].is_string() => {
                        "stdio"
                    }
                    Some(server) if server["url"].is_string() && !server["command"].is_string() => {
                        "http"
                    }
                    _ => "unknown",
                };
                Ok(Server {
                    name: name.clone(),
                    transport,
                    effective_enabled: reported_enabled(native)?,
                    in_user_config: stored.is_some(),
                    can_edit: stored.is_some() && valid_name(name) && preferences.target.is_some(),
                    user_transport: transport_of(stored),
                    saved_options: OptionKey::ALL
                        .into_iter()
                        .filter(|key| stored.is_some_and(|v| v.get(key.name()).is_some()))
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            target: preferences.target,
            servers: listed,
            observed,
        })
    }

    /// Host must bind a confirmation to the exact connection/view/directory.
    /// Snapshot revision is mandatory. Never automatically retry or reload.
    pub fn edit(&self, edit: &Edit) -> Result<Call, String> {
        let target = self
            .target
            .as_ref()
            .ok_or("No enabled base user configuration layer is available")?;
        let name = match edit {
            Edit::Add { name, .. }
            | Edit::SetEnabled { name, .. }
            | Edit::Remove { name }
            | Edit::SetOption { name, .. }
            | Edit::ClearOption { name, .. } => name,
        };
        if !valid_name(name) {
            return Err(
                "Use a server name containing only letters, digits, underscores and hyphens".into(),
            );
        }
        let key = format!("mcp_servers.{name}");
        let (key, value) = match edit {
            Edit::Add { transport, .. } => {
                if self.observed.contains(name) {
                    return Err("This MCP server name already exists in native configuration; nothing was overwritten".into());
                }
                (key, transport.configuration()?)
            }
            Edit::SetEnabled { enabled, .. } => {
                self.require_user_entry(name)?;
                (format!("{key}.enabled"), json!(enabled))
            }
            Edit::Remove { .. } => {
                self.require_user_entry(name)?;
                (key, Value::Null)
            }
            Edit::SetOption {
                key: option, value, ..
            } => {
                self.require_option(name, *option)?;
                option.validate(value)?;
                (format!("{key}.{}", option.name()), value.clone())
            }
            Edit::ClearOption { key: option, .. } => {
                let server = self.require_option(name, *option)?;
                if !option.clearable() || !server.saved_options.contains(option) {
                    return Err(
                        "This option is required or has no observed saved value to clear".into(),
                    );
                }
                (format!("{key}.{}", option.name()), Value::Null)
            }
        };
        let mut call = api::config_write(&target.file, &target.version, &key, value);
        if matches!(edit, Edit::SetOption { .. }) {
            // Replace this exact leaf, including maps; upsert would retain old
            // header entries contrary to the user's reviewed replacement.
            call.params["edits"][0]["mergeStrategy"] = json!("replace");
        }
        Ok(call)
    }
    fn require_user_entry(&self, name: &str) -> Result<(), String> {
        if !self
            .servers
            .iter()
            .any(|entry| entry.name == name && entry.can_edit)
        {
            return Err("This server has no editable base-user entry; project and managed layers are not modified here".into());
        }
        Ok(())
    }
    fn require_option(&self, name: &str, key: OptionKey) -> Result<&Server, String> {
        self.require_user_entry(name)?;
        self.servers.iter().find(|s| s.name==name && key.supports(s.user_transport))
            .ok_or_else(||"The saved user entry does not support this option. Its transport may differ from the effective configuration.".into())
    }
}

impl Transport {
    fn configuration(&self) -> Result<Value, String> {
        // Adding does not imply starting a process or connecting a service.
        // The UI's later explicit Enable action is separate from Save.
        let mut value = json!({"enabled":false});
        match self {
            Self::Stdio {
                command,
                args,
                cwd,
                env_vars,
            } => {
                if command.trim().is_empty()
                    || command.chars().any(char::is_control)
                    || args.iter().any(|arg| arg.contains('\0'))
                {
                    return Err("Enter a valid executable and argument list".into());
                }
                if !env_vars.iter().all(|name| env_name(name)) {
                    return Err("Enter environment variable names, never secret values".into());
                }
                if let Some(cwd) = cwd {
                    if cwd.contains('\0') || !Path::new(cwd).is_absolute() {
                        return Err(
                            "The MCP working directory must be an absolute local path".into()
                        );
                    }
                    value["cwd"] = json!(cwd);
                }
                value["command"] = json!(command);
                value["args"] = json!(args);
                value["env_vars"] = json!(env_vars);
            }
            Self::Http {
                url,
                bearer_token_env_var,
            } => {
                let parsed =
                    url::Url::parse(url).map_err(|_| "Enter a valid HTTP or HTTPS MCP URL")?;
                if !matches!(parsed.scheme(), "http" | "https")
                    || parsed.host_str().is_none()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.fragment().is_some()
                {
                    return Err(
                        "Use an HTTP(S) MCP URL without embedded credentials or a fragment".into(),
                    );
                }
                if let Some(name) = bearer_token_env_var {
                    if !env_name(name) {
                        return Err(
                            "Enter the bearer token's environment variable name, not the token"
                                .into(),
                        );
                    }
                    value["bearer_token_env_var"] = json!(name);
                }
                value["url"] = json!(url);
            }
        }
        Ok(value)
    }
}

/// A saved ACK is not evidence that a running MCP process reloaded.
pub fn saved(value: &Value, target: &Target) -> Result<bool, String> {
    if value["filePath"].as_str().map(Path::new) != Some(Path::new(&target.file))
        || value["version"].as_str().is_none_or(|v| v.is_empty())
    {
        return Err("Native write outcome is uncertain; refresh without retrying".into());
    }
    match value["status"].as_str() {
        Some("ok") => Ok(false),
        Some("okOverridden") => Ok(true),
        _ => Err("Unknown native write status; refresh without retrying".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        let path = std::env::current_dir().unwrap().join("config.toml");
        json!({"config":{"mcp_servers":{"existing":{"command":"secret-command","args":["secret-arg"],"env":{"TOKEN":"secret-env"},"enabled":true},"project":{"url":"https://example.invalid/?token=secret"}}},"origins":{},"layers":[{"name":{"type":"user","file":path,"profile":null},"version":"rev-1","disabledReason":null,"config":{"mcp_servers":{"existing":{"command":"secret-command","env":{"TOKEN":"secret-env"}}}}}]})
    }
    fn add(name: &str) -> Edit {
        Edit::Add {
            name: name.into(),
            transport: Transport::Stdio {
                command: "fixture-server".into(),
                args: vec!["--stdio".into()],
                cwd: None,
                env_vars: vec!["MCP_TOKEN".into()],
            },
        }
    }
    #[test]
    fn native_mcp_projection_never_serializes_commands_urls_or_credentials() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        let public = serde_json::to_string(&snapshot).unwrap();
        for private in ["secret", "example.invalid", "TOKEN"] {
            assert!(!public.contains(private), "{private}");
        }
        assert!(
            snapshot
                .servers
                .iter()
                .find(|s| s.name == "existing")
                .unwrap()
                .can_edit
        );
        assert!(
            !snapshot
                .servers
                .iter()
                .find(|s| s.name == "project")
                .unwrap()
                .can_edit
        );
        assert_eq!(
            format!("{:?}", add("new")),
            "NativeMcpConfigurationEdit(<redacted>)"
        );
    }
    #[test]
    fn native_mcp_edits_preserve_target_revision_and_unrelated_configuration() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        let call = snapshot.edit(&add("new-server")).unwrap();
        assert_eq!(call.method, "config/batchWrite");
        assert_eq!(call.params["expectedVersion"], "rev-1");
        assert_eq!(call.params["reloadUserConfig"], false);
        assert_eq!(call.params["edits"].as_array().unwrap().len(), 1);
        assert_eq!(call.params["edits"][0]["keyPath"], "mcp_servers.new-server");
        assert_eq!(call.params["edits"][0]["value"]["enabled"], false);
        let toggle = snapshot
            .edit(&Edit::SetEnabled {
                name: "existing".into(),
                enabled: false,
            })
            .unwrap();
        assert_eq!(
            toggle.params["edits"][0]["keyPath"],
            "mcp_servers.existing.enabled"
        );
        assert_eq!(toggle.params["edits"][0]["value"], false);
        let remove = snapshot
            .edit(&Edit::Remove {
                name: "existing".into(),
            })
            .unwrap();
        assert_eq!(remove.params["edits"][0]["keyPath"], "mcp_servers.existing");
        assert!(remove.params["edits"][0]["value"].is_null());
    }
    #[test]
    fn native_mcp_editor_rejects_collisions_paths_and_foreign_layers() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        for name in [
            "existing",
            "project",
            "",
            "new.enabled",
            "a[b]",
            "a/b",
            "a\\b",
        ] {
            assert!(snapshot.edit(&add(name)).is_err(), "{name}");
        }
        for name in ["project", "missing"] {
            assert!(snapshot.edit(&Edit::Remove { name: name.into() }).is_err());
            assert!(
                snapshot
                    .edit(&Edit::SetEnabled {
                        name: name.into(),
                        enabled: true
                    })
                    .is_err()
            );
        }
        let mut disabled = fixture();
        disabled["layers"][0]["disabledReason"] = json!("managed");
        assert!(
            Snapshot::read(&disabled)
                .unwrap()
                .edit(&add("new"))
                .is_err()
        );
        assert!(serde_json::from_value::<Edit>(json!({"kind":"add","name":"x","transport":{"kind":"stdio","command":"x","args":[],"env_vars":[],"env":{"TOKEN":"secret"}}})).is_err());
    }
    #[test]
    fn native_mcp_transports_validate_urls_and_environment_names() {
        for url in [
            "file:///C:/bad",
            "javascript:bad",
            "https://user:secret@example.invalid/mcp",
            "https://example.invalid/#token",
        ] {
            assert!(
                Transport::Http {
                    url: url.into(),
                    bearer_token_env_var: None
                }
                .configuration()
                .is_err()
            );
        }
        assert!(
            Transport::Http {
                url: "https://example.invalid/mcp".into(),
                bearer_token_env_var: Some("MCP_TOKEN".into())
            }
            .configuration()
            .is_ok()
        );
        for name in ["secret value", "A=B", "1TOKEN", "TOKEN.path", ""] {
            assert!(
                Transport::Http {
                    url: "https://example.invalid/mcp".into(),
                    bearer_token_env_var: Some(name.into())
                }
                .configuration()
                .is_err()
            );
        }
        let snapshot = Snapshot::read(&fixture()).unwrap();
        assert!(
            saved(
                &json!({"status":"ok","version":"rev-2","filePath":"other"}),
                snapshot.target.as_ref().unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn native_mcp_option_edits_are_single_leaf_and_user_transport_scoped() {
        let mut raw = fixture();
        // Effective transport must not authorize editing a different user layer.
        raw["config"]["mcp_servers"]["existing"] = json!({"url":"https://override.invalid"});
        raw["layers"][0]["config"]["mcp_servers"]["existing"]["args"] = json!(["private-arg"]);
        let snapshot = Snapshot::read(&raw).unwrap();
        let server = snapshot
            .servers
            .iter()
            .find(|s| s.name == "existing")
            .unwrap();
        assert_eq!(server.transport, "http");
        assert_eq!(server.user_transport, "stdio");
        let call = snapshot
            .edit(&Edit::SetOption {
                name: "existing".into(),
                key: OptionKey::Args,
                value: json!(["--new"]),
            })
            .unwrap();
        assert_eq!(
            call.params["edits"],
            json!([{"keyPath":"mcp_servers.existing.args","value":["--new"],"mergeStrategy":"replace"}])
        );
        assert_eq!(call.params["expectedVersion"], "rev-1");
        assert_eq!(call.params["reloadUserConfig"], false);
        assert!(!call.params.to_string().contains("secret"));
        assert!(
            snapshot
                .edit(&Edit::SetOption {
                    name: "existing".into(),
                    key: OptionKey::Url,
                    value: json!("https://new.invalid")
                })
                .is_err()
        );
        assert!(
            snapshot
                .edit(&Edit::SetOption {
                    name: "project".into(),
                    key: OptionKey::Url,
                    value: json!("https://new.invalid")
                })
                .is_err()
        );
        let clear = snapshot
            .edit(&Edit::ClearOption {
                name: "existing".into(),
                key: OptionKey::Args,
            })
            .unwrap();
        assert!(clear.params["edits"][0]["value"].is_null());
        for key in [OptionKey::Command, OptionKey::Url, OptionKey::Cwd] {
            assert!(
                snapshot
                    .edit(&Edit::ClearOption {
                        name: "existing".into(),
                        key
                    })
                    .is_err()
            );
        }
    }
    #[test]
    fn native_mcp_options_validate_types_without_accepting_secret_maps_or_key_paths() {
        for (key, value) in [
            (OptionKey::Args, json!(["", "with spaces"])),
            (OptionKey::EnvVars, json!(["TOKEN"])),
            (OptionKey::EnvHttpHeaders, json!({"X-Key":"TOKEN"})),
            (OptionKey::Required, json!(false)),
            (OptionKey::StartupTimeoutSec, json!(0.5)),
            (OptionKey::EnabledTools, json!([])),
        ] {
            key.validate(&value).unwrap();
        }
        for (key, value) in [
            (OptionKey::Args, Value::Null),
            (OptionKey::Args, json!({})),
            (OptionKey::EnvVars, json!(["TOKEN=private"])),
            (OptionKey::EnvHttpHeaders, json!({"X-Key":"private value"})),
            (OptionKey::EnvHttpHeaders, json!({"X\r\n-Key":"TOKEN"})),
            (OptionKey::ToolTimeoutSec, json!(0)),
            (OptionKey::ToolTimeoutSec, json!(-1)),
            (OptionKey::Required, json!("true")),
            (OptionKey::EnabledTools, json!([""])),
            (OptionKey::Cwd, json!("relative")),
        ] {
            assert!(key.validate(&value).is_err(), "{key:?}");
        }
        for key in [
            "env",
            "http_headers",
            "bearer_token",
            "command.args",
            "url\n",
        ] {
            assert!(
                serde_json::from_value::<Edit>(
                    json!({"kind":"set_option","name":"existing","key":key,"value":"private"})
                )
                .is_err()
            );
        }
    }
}
