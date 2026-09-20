//! Owner-scoped installed-plugin inventory and explicit composer mentions.
//!
//! The UI wire contract keeps its historical `apps` and `appId` names so an
//! in-flight frontend migration cannot break existing DOM/IPC hooks. The
//! authority is now `plugin/installed`: raw connector inventory contains
//! Codex control-plane services that are not user-facing plugins.
use super::*;
use crate::tab_context::sanitize_ui_text;

const MAX_PLUGINS: usize = 256;
const MAX_SELECTED_PLUGINS: usize = 16;
const MAX_MARKETPLACE_ERRORS: usize = 64;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AppsAction {
    Refresh {},
    Attach {
        inventory_id: String,
        app_id: String,
    },
    Remove {
        selection_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedApp {
    pub selection_id: String,
    /// Exact `name@marketplace` identifier returned by `plugin/installed`.
    pub app_id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub thread_id: String,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolView {
    name: String,
    title: Option<String>,
    description: String,
    enabled: bool,
    disabled_reason: Option<String>,
    read_only: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    id: String,
    name: String,
    description: Option<String>,
    icon_url: Option<String>,
    accessible: bool,
    configured_enabled: bool,
    installed: bool,
    runtime_enabled: bool,
    callable: bool,
    /// Retained for the existing settings wire shape. Plugin commands are
    /// resolved by Codex after the plugin mention, not inferred from MCP tools.
    tools: Vec<ToolView>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    thread_id: String,
    inventory_id: Option<String>,
    items: Vec<AppView>,
    selected: Vec<SelectedApp>,
    current: bool,
    busy: bool,
    error: Option<String>,
    unavailable_reason: Option<String>,
    notice: Option<String>,
}

#[derive(Clone)]
struct Plugin {
    id: String,
    name: String,
    description: Option<String>,
    icon_url: Option<String>,
    available: bool,
    enabled: bool,
}

struct Inventory {
    view: View,
    pending: Option<String>,
    plugins: BTreeMap<String, Plugin>,
    cwd: Option<String>,
    marketplace_errors: usize,
    dirty_during_refresh: bool,
    follow_up_used: bool,
}

impl Inventory {
    fn new(thread: &str) -> Self {
        Self {
            view: View {
                thread_id: thread.into(),
                ..View::default()
            },
            pending: None,
            plugins: BTreeMap::new(),
            cwd: None,
            marketplace_errors: 0,
            dirty_during_refresh: false,
            follow_up_used: false,
        }
    }

    fn begin(&mut self, call: Call) -> (String, Call) {
        let id = Uuid::new_v4().to_string();
        self.pending = Some(id.clone());
        self.view.busy = true;
        (id, call)
    }

    fn refresh(&mut self, cwd: Option<&str>) -> Result<(String, Call), String> {
        if self.pending.is_some() {
            return Err("A plugin inventory request is already pending".into());
        }
        self.view.current = false;
        self.view.error = None;
        self.view.unavailable_reason = None;
        self.view.notice = None;
        for selected in &mut self.view.selected {
            selected.current = false;
        }
        self.plugins.clear();
        self.cwd = cwd.map(str::to_owned);
        self.marketplace_errors = 0;
        self.dirty_during_refresh = false;
        self.follow_up_used = false;
        Ok(self.begin(api::plugins_installed(self.cwd.as_deref())))
    }

    fn action(
        &mut self,
        cwd: Option<&str>,
        action: AppsAction,
    ) -> Result<Option<(String, Call)>, String> {
        match action {
            AppsAction::Refresh {} => self.refresh(cwd).map(Some),
            AppsAction::Attach {
                inventory_id,
                app_id,
            } => {
                if self.pending.is_some()
                    || !self.view.current
                    || self.view.inventory_id.as_deref() != Some(&inventory_id)
                {
                    return Err(
                        "Refresh this conversation's plugin inventory before selecting a plugin"
                            .into(),
                    );
                }
                let plugin = self
                    .view
                    .items
                    .iter()
                    .find(|plugin| plugin.id == app_id)
                    .ok_or("This plugin is not in the current native inventory")?;
                if !(plugin.accessible
                    && plugin.configured_enabled
                    && plugin.installed
                    && plugin.runtime_enabled
                    && plugin.callable)
                {
                    return Err(
                        "This plugin is not currently available in the selected Codex conversation"
                            .into(),
                    );
                }
                if !self
                    .view
                    .selected
                    .iter()
                    .any(|item| item.app_id == plugin.id)
                {
                    if self.view.selected.len() >= MAX_SELECTED_PLUGINS {
                        return Err(format!(
                            "Select at most {MAX_SELECTED_PLUGINS} plugins for one native input"
                        ));
                    }
                    self.view.selected.push(SelectedApp {
                        selection_id: Uuid::new_v4().to_string(),
                        app_id: plugin.id.clone(),
                        name: plugin.name.clone(),
                        icon_url: plugin.icon_url.clone(),
                        thread_id: self.view.thread_id.clone(),
                        current: true,
                    });
                }
                Ok(None)
            }
            AppsAction::Remove { selection_id } => {
                self.view
                    .selected
                    .retain(|item| item.selection_id != selection_id);
                Ok(None)
            }
        }
    }

    fn installed_reply(&mut self, value: &Value) -> Result<Option<(String, Call)>, String> {
        let (plugins, marketplace_errors) = parse_plugins(value)?;
        self.plugins = plugins;
        self.marketplace_errors = marketplace_errors;
        if self.dirty_during_refresh && !self.follow_up_used {
            self.dirty_during_refresh = false;
            self.follow_up_used = true;
            self.plugins.clear();
            self.view.notice = Some(
                "Plugins changed while loading. Verifying the latest installed set once.".into(),
            );
            return Ok(Some(
                self.begin(api::plugins_installed(self.cwd.as_deref())),
            ));
        }
        let changed_again = self.dirty_during_refresh;
        self.dirty_during_refresh = false;
        self.publish();
        if changed_again {
            self.view.current = false;
            self.view.notice = Some(
                "Plugins changed repeatedly while loading. Refresh once before selecting one."
                    .into(),
            );
            for selected in &mut self.view.selected {
                selected.current = false;
            }
        }
        Ok(None)
    }

    fn publish(&mut self) {
        self.view.items = self
            .plugins
            .values()
            .map(|plugin| AppView {
                id: plugin.id.clone(),
                name: plugin.name.clone(),
                description: plugin.description.clone(),
                icon_url: plugin.icon_url.clone(),
                accessible: plugin.available,
                configured_enabled: plugin.enabled,
                installed: true,
                runtime_enabled: plugin.enabled,
                callable: plugin.available && plugin.enabled,
                tools: Vec::new(),
            })
            .collect();
        self.view.items.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        for selected in &mut self.view.selected {
            let current = self.view.items.iter().find(|plugin| {
                plugin.id == selected.app_id
                    && plugin.name == selected.name
                    && plugin.accessible
                    && plugin.configured_enabled
                    && plugin.installed
                    && plugin.runtime_enabled
                    && plugin.callable
            });
            selected.current = current.is_some();
            if let Some(plugin) = current {
                selected.icon_url = plugin.icon_url.clone();
            }
        }
        self.view.inventory_id = Some(Uuid::new_v4().to_string());
        self.view.current = true;
        self.view.busy = false;
        self.view.error = None;
        self.view.unavailable_reason = None;
        self.view.notice = Some(if self.marketplace_errors == 0 {
            "Installed user-facing plugins were read from Codex. Internal control-plane services are excluded."
                .into()
        } else {
            format!(
                "Installed plugins were loaded, but {} plugin marketplace{} could not be read.",
                self.marketplace_errors,
                if self.marketplace_errors == 1 {
                    ""
                } else {
                    "s"
                }
            )
        });
    }

    fn fail(&mut self, error: String) {
        self.plugins.clear();
        self.marketplace_errors = 0;
        self.dirty_during_refresh = false;
        self.follow_up_used = false;
        self.view.current = false;
        self.view.unavailable_reason = None;
        self.view.notice = None;
        self.view.error = Some(format!(
            "Native plugin inventory failed: {error}. Refresh explicitly; nothing was installed, called or retried."
        ));
        for selected in &mut self.view.selected {
            selected.current = false;
        }
    }

    fn reply(&mut self, id: &str, result: Result<Value, CallError>) -> Option<(String, Call)> {
        if self.pending.as_deref() != Some(id) {
            return None;
        }
        self.pending = None;
        self.view.busy = false;
        if let Err(error) = &result
            && let Some(reason) = plugins_unavailable_reason(error)
        {
            self.plugins.clear();
            self.marketplace_errors = 0;
            self.dirty_during_refresh = false;
            self.follow_up_used = false;
            self.view.current = false;
            self.view.error = None;
            self.view.unavailable_reason = Some(reason);
            self.view.notice = None;
            for selected in &mut self.view.selected {
                selected.current = false;
            }
            return None;
        }
        match result
            .map_err(public_plugins_call_error)
            .and_then(|value| self.installed_reply(&value))
        {
            Ok(next) => next,
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    fn invalidate(&mut self, notice: &str) {
        self.pending = None;
        self.plugins.clear();
        self.marketplace_errors = 0;
        self.dirty_during_refresh = false;
        self.follow_up_used = false;
        self.view.current = false;
        self.view.busy = false;
        self.view.error = None;
        self.view.unavailable_reason = None;
        self.view.notice = Some(notice.into());
        for selected in &mut self.view.selected {
            selected.current = false;
        }
    }

    fn mark_updated(&mut self, notice: &str) {
        if self.pending.is_some() {
            self.dirty_during_refresh = true;
            self.view.current = false;
            self.view.notice = Some(
                "Plugins changed while the installed inventory was loading. The snapshot will be verified automatically."
                    .into(),
            );
            for selected in &mut self.view.selected {
                selected.current = false;
            }
        } else {
            self.invalidate(notice);
        }
    }
}

#[derive(Default)]
pub(super) struct Apps {
    owners: BTreeMap<String, Inventory>,
}

impl Apps {
    fn scope(thread: Option<&str>) -> &str {
        thread.unwrap_or("")
    }

    pub(super) fn view(&self, owner: &str, thread: Option<&str>) -> Option<&View> {
        let thread = Self::scope(thread);
        self.owners
            .get(owner)
            .filter(|inventory| inventory.view.thread_id == thread)
            .map(|inventory| &inventory.view)
    }

    fn inventory_mut(&mut self, owner: &str, thread: &str) -> &mut Inventory {
        let replace = self
            .owners
            .get(owner)
            .is_some_and(|inventory| inventory.view.thread_id != thread);
        if replace {
            self.owners.remove(owner);
        }
        self.owners
            .entry(owner.into())
            .or_insert_with(|| Inventory::new(thread))
    }

    fn action(
        &mut self,
        owner: &str,
        thread: Option<&str>,
        cwd: Option<&str>,
        action: AppsAction,
    ) -> Result<Option<(String, Call)>, String> {
        let thread = Self::scope(thread);
        self.inventory_mut(owner, thread).action(cwd, action)
    }

    fn reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) -> Option<(String, Call)> {
        self.owners
            .get_mut(owner)
            .filter(|inventory| inventory.view.thread_id == thread)?
            .reply(id, result)
    }

    pub(super) fn capture(
        &self,
        owner: &str,
        thread: Option<&str>,
    ) -> Result<Vec<SelectedApp>, String> {
        let thread = Self::scope(thread);
        let Some(inventory) = self
            .owners
            .get(owner)
            .filter(|inventory| inventory.view.thread_id == thread)
        else {
            return Ok(Vec::new());
        };
        if inventory
            .view
            .selected
            .iter()
            .any(|selected| !selected.current || selected.thread_id != thread)
        {
            return Err(
                "A selected Codex plugin is stale. Refresh Plugins or remove it before sending."
                    .into(),
            );
        }
        Ok(inventory.view.selected.clone())
    }

    pub(super) fn promote(&mut self, owner: &str, thread: &str) {
        let Some(inventory) = self.owners.get_mut(owner) else {
            return;
        };
        if inventory.view.thread_id.is_empty() && !thread.is_empty() {
            inventory.view.thread_id = thread.into();
            for selected in &mut inventory.view.selected {
                selected.thread_id = thread.into();
            }
        }
    }

    pub(super) fn consume(&mut self, owner: &str, accepted: &[SelectedApp]) {
        if let Some(inventory) = self.owners.get_mut(owner) {
            inventory.view.selected.retain(|selected| {
                !accepted
                    .iter()
                    .any(|accepted| accepted.selection_id == selected.selection_id)
            });
        }
    }

    pub(super) fn invalidate_all(&mut self, notice: &str) -> Vec<String> {
        self.owners
            .iter_mut()
            .map(|(owner, inventory)| {
                inventory.invalidate(notice);
                owner.clone()
            })
            .collect()
    }

    pub(super) fn list_updated(&mut self, notice: &str) -> Vec<String> {
        self.owners
            .iter_mut()
            .map(|(owner, inventory)| {
                inventory.mark_updated(notice);
                owner.clone()
            })
            .collect()
    }

    pub(super) fn invalidate_thread(&mut self, thread: &str, notice: &str) -> Vec<String> {
        self.owners
            .iter_mut()
            .filter(|(_, inventory)| inventory.view.thread_id == thread)
            .map(|(owner, inventory)| {
                inventory.invalidate(notice);
                owner.clone()
            })
            .collect()
    }

    pub(super) fn remove_owner(&mut self, owner: &str) {
        self.owners.remove(owner);
    }
}

impl State {
    pub(in crate::browser) fn capture_apps(
        &self,
        owner: &str,
        thread: Option<&str>,
    ) -> Result<Vec<SelectedApp>, String> {
        self.apps.capture(owner, thread)
    }

    pub(in crate::browser) fn promote_apps(&mut self, owner: &str, thread: &str) {
        self.apps.promote(owner, thread);
    }

    pub(in crate::browser) fn consume_apps(&mut self, owner: &str, accepted: &[SelectedApp]) {
        self.apps.consume(owner, accepted);
    }
}

impl BrowserApp {
    pub(super) fn control_native_apps(
        &mut self,
        owner: &str,
        thread: Option<&str>,
        action: AppsAction,
    ) -> Result<(), String> {
        let binding = self.app_server.conversations.binding(owner);
        match (thread, binding) {
            (None, None) => {}
            (Some(expected), Some(binding))
                if !expected.is_empty()
                    && binding.thread_id == expected
                    && !binding.archived
                    && !binding.deleted => {}
            _ => {
                return Err(
                    "The native conversation changed. Reopen Plugins before continuing.".into(),
                );
            }
        }
        let target = self.conversation_target(owner)?;
        if target.remote {
            return Err("Select Codex in a local conversation to use Plugins".into());
        }
        let cwd = target
            .root
            .or_else(|| self.app_server.roots.get(owner).cloned())
            .and_then(|root| root.to_str().map(str::to_owned));
        let result = self
            .app_server
            .apps
            .action(owner, thread, cwd.as_deref(), action)?;
        if let Some((id, call)) = result {
            self.native_apps_call(owner.into(), Apps::scope(thread).into(), id, call);
        }
        self.emit_app_server_conversation(owner, "stream");
        Ok(())
    }

    fn native_apps_call(&self, owner: String, thread: String, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::AppsReply {
                epoch,
                owner,
                thread,
                id,
                result,
            }));
        });
    }

    pub(super) fn native_apps_reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        let binding = self.app_server.conversations.binding(owner);
        let current = if thread.is_empty() {
            binding.is_none()
        } else {
            binding.is_some_and(|binding| binding.thread_id == thread && !binding.deleted)
        };
        if !current {
            return;
        }
        if let Some((next, call)) = self.app_server.apps.reply(owner, thread, id, result) {
            self.native_apps_call(owner.into(), thread.into(), next, call);
        }
        self.emit_app_server_conversation(owner, "stream");
    }
}

pub(super) fn append_app_inputs(
    input: &mut Vec<Value>,
    plugins: &[SelectedApp],
) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut mentions = Vec::with_capacity(plugins.len());
    for plugin in plugins {
        if !plugin.current
            || !valid_plugin_id(&plugin.app_id)
            || !valid_text(&plugin.name, 512)
            || !ids.insert(&plugin.app_id)
        {
            return Err("A selected native plugin mention is invalid or stale".into());
        }
        mentions.push(format!("@{}", plugin_slug(&plugin.app_id).unwrap()));
    }
    if !mentions.is_empty() {
        let prefix = mentions.join(" ");
        if let Some(text) = input
            .iter()
            .find(|item| item["type"] == "text")
            .and_then(|item| item.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            if let Some(item) = input.iter_mut().find(|item| item["type"] == "text") {
                item["text"] = json!(if text.is_empty() {
                    prefix
                } else {
                    format!("{prefix} {text}")
                });
            }
        } else {
            input.insert(0, api::text_input(&prefix));
        }
    }
    for plugin in plugins {
        input.push(api::plugin_mention_input(&plugin.name, &plugin.app_id));
    }
    Ok(())
}

fn parse_plugins(value: &Value) -> Result<(BTreeMap<String, Plugin>, usize), String> {
    let marketplaces = value
        .get("marketplaces")
        .and_then(Value::as_array)
        .ok_or("Installed plugin response is incomplete")?;
    let marketplace_errors = value
        .get("marketplaceLoadErrors")
        .and_then(Value::as_array)
        .ok_or("Installed plugin response is incomplete")?;
    if marketplace_errors.len() > MAX_MARKETPLACE_ERRORS {
        return Err("Installed plugin response contains too many marketplace errors".into());
    }
    let mut result = BTreeMap::new();
    let mut observed = 0usize;
    for marketplace in marketplaces {
        required_text(marketplace, "name", 512)?;
        let plugins = marketplace
            .get("plugins")
            .and_then(Value::as_array)
            .ok_or("Installed plugin marketplace is incomplete")?;
        observed = observed.saturating_add(plugins.len());
        if observed > MAX_PLUGINS {
            return Err(
                "The installed plugin inventory exceeds the supported display limit".into(),
            );
        }
        for raw in plugins {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| valid_plugin_id(id))
                .map(str::to_owned)
                .ok_or("Installed plugin response contains an invalid id")?;
            let installed = required_bool(raw, "installed")?;
            let enabled = required_bool(raw, "enabled")?;
            let install_policy = required_text(raw, "installPolicy", 64)?;
            if !matches!(
                install_policy.as_str(),
                "AVAILABLE" | "INSTALLED_BY_DEFAULT" | "NOT_AVAILABLE"
            ) {
                return Err("Installed plugin response contains an invalid install policy".into());
            }
            let availability = required_text(raw, "availability", 64)?;
            if !matches!(availability.as_str(), "AVAILABLE" | "DISABLED_BY_ADMIN") {
                return Err(
                    "Installed plugin response contains an invalid availability state".into(),
                );
            }
            let interface = match raw.get("interface") {
                None | Some(Value::Null) => None,
                Some(Value::Object(_)) => raw.get("interface"),
                _ => {
                    return Err(
                        "Installed plugin response contains invalid interface metadata".into(),
                    );
                }
            };
            let raw_name = required_text(raw, "name", 512)?;
            let name = interface
                .map(|interface| optional_text(interface, "displayName", 512))
                .transpose()?
                .flatten()
                .unwrap_or(raw_name);
            let description = interface
                .map(|interface| optional_text(interface, "shortDescription", 8 * 1024))
                .transpose()?
                .flatten()
                .or(interface
                    .map(|interface| optional_text(interface, "longDescription", 16 * 1024))
                    .transpose()?
                    .flatten());
            let icon_url = interface.and_then(plugin_icon_url);
            // Installed-by-default packages are Codex infrastructure. They can
            // supply services such as plugin discovery, templates and safety,
            // but Codex does not present them as ordinary composer plugins.
            if !installed || install_policy == "INSTALLED_BY_DEFAULT" {
                continue;
            }
            let plugin = Plugin {
                id: id.clone(),
                name,
                description,
                icon_url,
                available: availability == "AVAILABLE" && install_policy != "NOT_AVAILABLE",
                enabled,
            };
            if result.insert(id, plugin).is_some() {
                return Err("Installed plugin response contains a repeated plugin ID".into());
            }
        }
    }
    Ok((result, marketplace_errors.len()))
}

fn required_bool(value: &Value, key: &str) -> Result<bool, String> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Installed plugin response contains an invalid {key}"))
}

fn required_text(value: &Value, key: &str, maximum: usize) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| valid_text(text, maximum))
        .map(str::to_owned)
        .ok_or_else(|| format!("Installed plugin response contains an invalid {key}"))
}

fn optional_text(value: &Value, key: &str, maximum: usize) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if valid_text(text, maximum) => Ok(Some(text.clone())),
        _ => Err(format!(
            "Installed plugin response contains an invalid {key}"
        )),
    }
}

/// Official remote plugins currently expose composer marks through OpenAI's
/// content host. Keep those URLs bounded, credential-free and HTTPS-only so an
/// installed third-party marketplace cannot turn the plugin menu into an
/// arbitrary tracking-image surface. Other plugins retain the local identity
/// fallback in the UI.
fn plugin_icon_url(interface: &Value) -> Option<String> {
    let value = interface.get("composerIconUrl")?.as_str()?;
    if value.is_empty() || value.len() > 8 * 1024 || value.contains('\0') {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    (parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.host_str() == Some("files.openai.com")
        && parsed.port_or_known_default() == Some(443)
        && parsed.fragment().is_none())
    .then(|| parsed.to_string())
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.contains('\0')
}

fn plugin_slug(value: &str) -> Option<&str> {
    let (plugin, marketplace) = value.split_once('@')?;
    (!plugin.is_empty() && !marketplace.is_empty() && !marketplace.contains('@')).then_some(plugin)
}

fn valid_plugin_id(value: &str) -> bool {
    if value.is_empty() || value.len() > 512 || plugin_slug(value).is_none() {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@'))
}

fn public_plugins_call_error(error: CallError) -> String {
    match error {
        CallError::Rpc(error) => {
            let summary = public_error_summary(&error.message, 240);
            if summary.is_empty() {
                format!(
                    "App Server rejected the plugin inventory request (RPC {})",
                    error.code
                )
            } else {
                format!("{summary} (RPC {})", error.code)
            }
        }
        CallError::Rejected(reason) => {
            let summary = public_error_summary(&reason, 320);
            if summary.is_empty() {
                "The plugin inventory request was rejected without a public error message".into()
            } else {
                summary
            }
        }
        CallError::Disconnected {
            reason,
            delivery_unknown,
        } => {
            let summary = public_error_summary(&reason, 320);
            let summary = if summary.is_empty() {
                "The App Server connection closed while loading plugins".into()
            } else {
                summary
            };
            if delivery_unknown {
                format!("{summary}; request acceptance is unknown")
            } else {
                summary
            }
        }
    }
}

fn plugins_unavailable_reason(error: &CallError) -> Option<String> {
    let CallError::Rpc(error) = error else {
        return None;
    };
    let message = error.message.to_ascii_lowercase();
    [
        "status 401",
        "401 unauthorized",
        "http 401",
        "status 403",
        "403 forbidden",
        "http 403",
    ]
    .iter()
    .any(|marker| message.contains(marker))
    .then(|| {
        "Codex plugins are unavailable for this ChatGPT account or workspace. App Server denied access to the installed plugin inventory; no plugin was selected, installed or called."
            .into()
    })
}

fn public_error_summary(value: &str, maximum: usize) -> String {
    let prefix = value
        .split(['<', '\r', '\n'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(':');
    let compact = prefix.split_whitespace().collect::<Vec<_>>().join(" ");
    sanitize_ui_text(&compact, maximum).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(
        id: &str,
        name: &str,
        display_name: Option<&str>,
        install_policy: &str,
        enabled: bool,
        availability: &str,
    ) -> Value {
        json!({
            "id":id,
            "remotePluginId":null,
            "version":"1.0.0",
            "localVersion":"1.0.0",
            "name":name,
            "shareContext":null,
            "source":{"type":"remote"},
            "installed":true,
            "installedAt":null,
            "enabled":enabled,
            "installPolicy":install_policy,
            "installPolicySource":null,
            "mustShowInstallationInterstitial":null,
            "authPolicy":"ON_INSTALL",
            "availability":availability,
            "disabledReason":null,
            "eligiblePlanTypes":null,
            "interface":display_name.map(|display_name| json!({
                "displayName":display_name,
                "shortDescription":format!("{display_name} integration"),
                "longDescription":null,
                "composerIconUrl":format!("https://files.openai.com/content/{name}.png")
            })),
            "keywords":[]
        })
    }

    fn response(plugins: Vec<Value>) -> Value {
        json!({
            "marketplaces":[{
                "name":"openai-curated-remote",
                "path":null,
                "interface":null,
                "plugins":plugins
            }],
            "marketplaceLoadErrors":[]
        })
    }

    fn refresh(inventory: &mut Inventory) -> String {
        let (id, call) = inventory.refresh(Some("C:\\workspace")).unwrap();
        assert_eq!(call.method, "plugin/installed");
        assert_eq!(call.params["cwds"], json!(["C:\\workspace"]));
        assert!(call.params["installSuggestionPluginNames"].is_null());
        id
    }

    fn attach(
        inventory: &mut Inventory,
        plugin_id: &str,
    ) -> Result<Option<(String, Call)>, String> {
        inventory.action(
            Some("C:\\workspace"),
            AppsAction::Attach {
                inventory_id: inventory.view.inventory_id.clone().unwrap_or_default(),
                app_id: plugin_id.into(),
            },
        )
    }

    #[test]
    fn draft_selection_is_promoted_to_the_first_native_thread() {
        let mut apps = Apps::default();
        let (id, call) = apps
            .action(
                "graph:agent-a",
                None,
                Some("C:\\workspace"),
                AppsAction::Refresh {},
            )
            .unwrap()
            .unwrap();
        assert_eq!(call.method, "plugin/installed");
        assert!(
            apps.reply(
                "graph:agent-a",
                "",
                &id,
                Ok(response(vec![plugin(
                    "github@openai-curated-remote",
                    "github",
                    Some("GitHub"),
                    "AVAILABLE",
                    true,
                    "AVAILABLE",
                )])),
            )
            .is_none()
        );
        let inventory_id = apps
            .view("graph:agent-a", None)
            .and_then(|view| view.inventory_id.clone())
            .unwrap();
        apps.action(
            "graph:agent-a",
            None,
            Some("C:\\workspace"),
            AppsAction::Attach {
                inventory_id,
                app_id: "github@openai-curated-remote".into(),
            },
        )
        .unwrap();

        let frozen = apps.capture("graph:agent-a", None).unwrap();
        assert_eq!(frozen.len(), 1);
        assert!(frozen[0].thread_id.is_empty());
        let mut input = vec![api::text_input("Inspect this repository")];
        append_app_inputs(&mut input, &frozen).unwrap();
        assert_eq!(input[0]["text"], "@github Inspect this repository");

        apps.promote("graph:agent-a", "thread-first");
        assert!(apps.view("graph:agent-a", None).is_none());
        let promoted = apps.view("graph:agent-a", Some("thread-first")).unwrap();
        assert_eq!(promoted.selected[0].thread_id, "thread-first");
        apps.consume("graph:agent-a", &frozen);
        assert!(
            apps.view("graph:agent-a", Some("thread-first"))
                .unwrap()
                .selected
                .is_empty()
        );
    }

    #[test]
    fn inventory_uses_user_plugins_and_emits_exact_plugin_mentions() {
        let mut inventory = Inventory::new("thread-a");
        let id = refresh(&mut inventory);
        assert!(
            inventory
                .reply(
                    &id,
                    Ok(response(vec![
                        plugin(
                            "github@openai-curated-remote",
                            "github",
                            Some("GitHub"),
                            "AVAILABLE",
                            true,
                            "AVAILABLE"
                        ),
                        plugin(
                            "cloudflare@openai-curated-remote",
                            "cloudflare",
                            Some("Cloudflare"),
                            "AVAILABLE",
                            true,
                            "AVAILABLE"
                        ),
                        plugin(
                            "plugin-management@openai-curated-remote",
                            "plugin-management",
                            Some("Plugin Management"),
                            "INSTALLED_BY_DEFAULT",
                            true,
                            "AVAILABLE"
                        ),
                    ])),
                )
                .is_none()
        );
        assert!(inventory.view.current);
        assert_eq!(
            inventory
                .view
                .items
                .iter()
                .map(|plugin| plugin.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Cloudflare", "GitHub"]
        );
        assert!(
            inventory
                .view
                .items
                .iter()
                .all(|plugin| plugin.name != "Plugin Management")
        );
        assert_eq!(
            inventory.view.items[0].icon_url.as_deref(),
            Some("https://files.openai.com/content/cloudflare.png")
        );

        attach(&mut inventory, "github@openai-curated-remote").unwrap();
        assert_eq!(
            inventory.view.selected[0].icon_url.as_deref(),
            Some("https://files.openai.com/content/github.png")
        );
        let mut input = vec![api::text_input("Inspect my repository")];
        append_app_inputs(&mut input, &inventory.view.selected).unwrap();
        assert_eq!(input[0]["text"], "@github Inspect my repository");
        assert_eq!(
            input[1],
            json!({
                "type":"mention",
                "name":"GitHub",
                "path":"plugin://github@openai-curated-remote"
            })
        );
        inventory.invalidate("changed");
        assert!(append_app_inputs(&mut input, &inventory.view.selected).is_err());
    }

    #[test]
    fn disabled_or_admin_blocked_plugins_are_visible_but_not_selectable() {
        let mut inventory = Inventory::new("thread-a");
        let id = refresh(&mut inventory);
        inventory.reply(
            &id,
            Ok(response(vec![
                plugin(
                    "disabled@personal",
                    "disabled",
                    Some("Disabled"),
                    "AVAILABLE",
                    false,
                    "AVAILABLE",
                ),
                plugin(
                    "blocked@workspace",
                    "blocked",
                    Some("Blocked"),
                    "AVAILABLE",
                    true,
                    "DISABLED_BY_ADMIN",
                ),
            ])),
        );
        assert_eq!(inventory.view.items.len(), 2);
        assert!(attach(&mut inventory, "disabled@personal").is_err());
        assert!(attach(&mut inventory, "blocked@workspace").is_err());
        assert!(inventory.view.selected.is_empty());
    }

    #[test]
    fn refresh_race_gets_one_bounded_follow_up() {
        let mut inventory = Inventory::new("thread-a");
        let first = refresh(&mut inventory);
        inventory.mark_updated("changed");
        let (second, call) = inventory.reply(&first, Ok(response(Vec::new()))).unwrap();
        assert_eq!(call.method, "plugin/installed");
        assert!(!inventory.view.current);
        inventory.mark_updated("changed again");
        assert!(inventory.reply(&second, Ok(response(Vec::new()))).is_none());
        assert!(!inventory.view.current);
        assert!(
            inventory
                .view
                .notice
                .as_deref()
                .unwrap()
                .contains("changed repeatedly")
        );
    }

    #[test]
    fn access_denial_is_bounded_and_redacted() {
        let mut inventory = Inventory::new("thread-a");
        let id = refresh(&mut inventory);
        assert!(
            inventory
                .reply(
                    &id,
                    Err(CallError::Rpc(RpcError {
                        code: -32000,
                        message: "Request failed with status 403 Forbidden: <html>private@example.com</html>"
                            .into(),
                        data: Some(json!({"secret":"hidden"})),
                    })),
                )
                .is_none()
        );
        let reason = inventory.view.unavailable_reason.as_deref().unwrap();
        assert!(reason.contains("plugins are unavailable"));
        assert!(!reason.contains("private@example.com"));
        assert!(inventory.view.error.is_none());
        assert!(!inventory.view.current);
    }

    #[test]
    fn malformed_or_duplicate_plugin_ids_fail_closed() {
        for plugins in [
            vec![plugin(
                "missing-marketplace",
                "bad",
                None,
                "AVAILABLE",
                true,
                "AVAILABLE",
            )],
            vec![
                plugin(
                    "same@personal",
                    "same",
                    None,
                    "AVAILABLE",
                    true,
                    "AVAILABLE",
                ),
                plugin(
                    "same@personal",
                    "same",
                    None,
                    "AVAILABLE",
                    true,
                    "AVAILABLE",
                ),
            ],
        ] {
            let mut inventory = Inventory::new("thread-a");
            let id = refresh(&mut inventory);
            assert!(inventory.reply(&id, Ok(response(plugins))).is_none());
            assert!(!inventory.view.current);
            assert!(inventory.view.error.is_some());
        }
        assert!(!valid_plugin_id("plugin@marketplace@unexpected"));
        assert!(!valid_plugin_id("plugin\nignore@marketplace"));
    }

    #[test]
    fn plugin_icons_accept_only_bounded_openai_https_content_urls() {
        for bad in [
            "http://files.openai.com/icon.png",
            "https://user@files.openai.com/icon.png",
            "https://files.openai.com.evil.test/icon.png",
            "https://example.test/icon.png",
            "https://files.openai.com/icon.png#private",
        ] {
            assert!(plugin_icon_url(&json!({"composerIconUrl":bad})).is_none());
        }
        assert_eq!(
            plugin_icon_url(&json!({
                "composerIconUrl":"https://files.openai.com/content?id=icon&cdn=1"
            }))
            .as_deref(),
            Some("https://files.openai.com/content?id=icon&cdn=1")
        );
        assert!(plugin_icon_url(&json!({"composerIconUrl":null})).is_none());
    }
}
