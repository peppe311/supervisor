//! Native configured-server inventory, versioned settings, OAuth and explicit
//! direct resource/tool operations. No custom tool registration or model prompt.
use super::*;
use central_agent_codex_runtime::{
    configuration::Target,
    mcp_configuration::{self, Edit, Snapshot},
};

const MAX_MCP_SERVERS: usize = 512;
const MAX_JSON_DEPTH: usize = 8;
const MAX_JSON_NODES: usize = 1024;
// Native MCP schemas can legitimately be deeper than direct arguments or
// returned structured content. Keep their larger read-only inspection budget
// separate so accepting a generated schema does not loosen call/result bounds.
const MAX_MCP_SCHEMA_DEPTH: usize = 16;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum McpAction {
    InspectConfiguration {},
    ChangeConfiguration {
        view_id: String,
        edit: Edit,
    },
    Refresh {},
    Reload {
        inventory_id: String,
    },
    Login {
        inventory_id: String,
        name: String,
    },
    OpenLogin {
        attempt_id: String,
    },
    ReadResource {
        inventory_id: String,
        server: String,
        resource_id: String,
    },
    CallTool {
        inventory_id: String,
        server: String,
        tool: String,
        arguments: Value,
    },
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    configuration: Option<ConfigurationView>,
    pub(super) thread_id: Option<String>,
    pub(super) inventory_id: Option<String>,
    pub(super) servers: Vec<Server>,
    pub(super) current: bool,
    pub(super) busy: bool,
    pub(super) login: Option<LoginView>,
    pub(super) error: Option<String>,
    pub(super) notice: Option<String>,
    pub(super) direct_result: Option<DirectResult>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationView {
    view_id: String,
    current: bool,
    snapshot: Snapshot,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Server {
    name: String,
    runtime_status: Option<String>,
    auth_status: String,
    tools: BTreeMap<String, Entry>,
    resources: Vec<Entry>,
    resource_templates: Vec<Entry>,
}
// Only bounded names/descriptions, opaque handles and sanitized display schemas
// cross into the UI; no raw schemas, resource content, URI credentials,
// environment configuration, icons or arbitrary server metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Entry {
    #[serde(default, skip_deserializing)]
    entry_id: String,
    name: String,
    description: Option<String>,
    #[serde(default, skip_serializing)]
    uri: Option<String>,
    #[serde(default, rename = "inputSchema", skip_serializing)]
    raw_input_schema: Option<Value>,
    #[serde(default, skip_deserializing)]
    input_schema: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DirectResult {
    operation: &'static str,
    server: String,
    name: String,
    contents: Vec<DirectContent>,
    structured_content: Option<String>,
    is_error: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectContent {
    kind: &'static str,
    mime_type: Option<String>,
    text: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoginView {
    attempt_id: String,
    name: String,
    origin: Option<String>,
    can_open: bool,
}
struct LoginAttempt {
    id: String,
    name: String,
    url: Option<url::Url>,
    acknowledged: bool,
    completed: Option<bool>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    List,
    Reload,
    Login,
    ReadConfiguration,
    WriteConfiguration,
    ReadResource,
    CallTool,
}
struct DirectTarget {
    server: String,
    name: String,
    uri: Option<String>,
}
#[derive(Default)]
pub(super) struct McpState {
    pub(super) view: View,
    pending: Option<(String, Operation)>,
    pages: BTreeMap<String, Server>,
    cursors: HashSet<String>,
    login: Option<LoginAttempt>,
    write_target: Option<Target>,
    direct_target: Option<DirectTarget>,
}
impl McpState {
    pub(super) fn writing(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, op)| *op == Operation::WriteConfiguration)
    }
    pub(super) fn direct_tool_pending(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, operation)| *operation == Operation::CallTool)
    }
    pub(super) fn stale_configuration(&mut self) {
        if let Some(config) = &mut self.view.configuration {
            config.current = false;
        }
        // A read started before a shared preferences/skills write must not
        // publish its old snapshot as current after that write invalidates it.
        // Discard only the local read receipt; never discard a pending write ACK.
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, op)| *op == Operation::ReadConfiguration)
        {
            self.pending = None;
            self.view.busy = false;
            self.view.notice = Some("Shared configuration changed during inspection. Refresh MCP configuration before editing.".into());
        }
    }
    pub(super) fn for_thread(thread: &str) -> Self {
        Self {
            view: View {
                thread_id: Some(thread.into()),
                ..View::default()
            },
            ..Self::default()
        }
    }
    fn list_call(&self, cursor: Option<&str>) -> Call {
        match self.view.thread_id.as_deref() {
            Some(thread) => api::thread_mcp_status(thread, cursor),
            None => api::mcp_status(cursor),
        }
    }
    pub(super) fn refresh(&mut self) -> Result<(String, Call), String> {
        self.action(McpAction::Refresh {})
    }
    fn begin(&mut self, op: Operation, call: Call) -> (String, Call) {
        let id = uuid::Uuid::new_v4().to_string();
        self.pending = Some((id.clone(), op));
        self.view.busy = true;
        (id, call)
    }
    fn list(&mut self) -> (String, Call) {
        self.view.current = false;
        self.pages.clear();
        self.cursors.clear();
        self.begin(Operation::List, self.list_call(None))
    }
    fn check_inventory(&self, id: &str) -> Result<(), String> {
        if !self.view.current || self.view.inventory_id.as_deref() != Some(id) {
            return Err("The configured-server list changed. Refresh it and choose again.".into());
        }
        Ok(())
    }
    pub(super) fn action(&mut self, action: McpAction) -> Result<(String, Call), String> {
        if self.view.thread_id.is_some()
            && matches!(
                action,
                McpAction::Reload { .. }
                    | McpAction::InspectConfiguration {}
                    | McpAction::ChangeConfiguration { .. }
            )
        {
            return Err("Shared MCP configuration reload belongs in Codex Settings.".into());
        }
        if self.pending.is_some() || self.login.is_some() {
            return Err("An MCP request or sign-in is still pending. Finish it first; reconnecting discards this client's pending state without replaying it.".into());
        }
        self.view.error = None;
        self.view.notice = None;
        self.view.direct_result = None;
        match action {
            McpAction::InspectConfiguration {} => {
                self.stale_configuration();
                Ok(self.begin(
                    Operation::ReadConfiguration,
                    Call {
                        method: "config/read",
                        params: json!({"includeLayers":true}),
                    },
                ))
            }
            McpAction::ChangeConfiguration { view_id, edit } => {
                let observed = self
                    .view
                    .configuration
                    .as_ref()
                    .filter(|view| view.current && view.view_id == view_id)
                    .ok_or("Refresh MCP configuration and review the change again")?;
                let call = observed.snapshot.edit(&edit)?;
                self.write_target = observed.snapshot.target.clone();
                self.stale_configuration();
                self.view.current = false;
                Ok(self.begin(Operation::WriteConfiguration, call))
            }
            McpAction::Refresh {} => Ok(self.list()),
            McpAction::Reload { inventory_id } => {
                self.check_inventory(&inventory_id)?;
                self.view.current = false;
                Ok(self.begin(Operation::Reload, api::mcp_reload()))
            }
            McpAction::Login { inventory_id, name } => {
                self.check_inventory(&inventory_id)?;
                let server = self
                    .view
                    .servers
                    .iter()
                    .find(|s| s.name == name)
                    .ok_or("This MCP server is not in the current inventory")?;
                if !can_login(server) {
                    return Err("This server does not report OAuth sign-in support.".into());
                }
                let call = match self.view.thread_id.as_deref() {
                    Some(thread) => api::thread_mcp_login(thread, &name),
                    None => api::mcp_login(&name),
                };
                let request = self.begin(Operation::Login, call);
                self.login = Some(LoginAttempt {
                    id: request.0.clone(),
                    name,
                    url: None,
                    acknowledged: false,
                    completed: None,
                });
                self.sync_login();
                Ok(request)
            }
            McpAction::OpenLogin { .. } => {
                Err("Opening a sign-in page is not an RPC request".into())
            }
            McpAction::ReadResource {
                inventory_id,
                server,
                resource_id,
            } => {
                self.check_inventory(&inventory_id)?;
                let resource = self
                    .view
                    .servers
                    .iter()
                    .find(|entry| entry.name == server)
                    .and_then(|entry| {
                        entry
                            .resources
                            .iter()
                            .find(|resource| resource.entry_id == resource_id)
                    })
                    .ok_or("This MCP resource is not in the current inventory")?;
                let uri = resource
                    .uri
                    .clone()
                    .ok_or("This MCP resource has no readable URI")?;
                self.direct_target = Some(DirectTarget {
                    server: server.clone(),
                    name: resource.name.clone(),
                    uri: Some(uri.clone()),
                });
                Ok(self.begin(
                    Operation::ReadResource,
                    api::mcp_resource_read(self.view.thread_id.as_deref(), &server, &uri),
                ))
            }
            McpAction::CallTool {
                inventory_id,
                server,
                tool,
                arguments,
            } => {
                self.check_inventory(&inventory_id)?;
                let thread = self
                    .view
                    .thread_id
                    .as_deref()
                    .ok_or("Direct MCP tool calls require an exact loaded conversation")?;
                let entry = self
                    .view
                    .servers
                    .iter()
                    .find(|entry| entry.name == server)
                    .and_then(|entry| entry.tools.get(&tool))
                    .ok_or("This MCP tool is not in the current conversation inventory")?;
                validate_arguments(&arguments)?;
                self.direct_target = Some(DirectTarget {
                    server: server.clone(),
                    name: entry.name.clone(),
                    uri: None,
                });
                Ok(self.begin(
                    Operation::CallTool,
                    api::mcp_tool_call(thread, &server, &tool, arguments),
                ))
            }
        }
    }
    fn page(&mut self, value: Value) -> Result<Option<(String, Call)>, String> {
        let cursor = value
            .get("nextCursor")
            .ok_or("Incomplete MCP pagination response")?;
        let next = if cursor.is_null() {
            None
        } else {
            Some(
                cursor
                    .as_str()
                    .filter(|cursor| bounded_text(cursor, 16 * 1024))
                    .ok_or("Invalid MCP cursor")?
                    .to_owned(),
            )
        };
        let mut data: Vec<Server> =
            serde_json::from_value(value.get("data").ok_or("Incomplete MCP inventory")?.clone())
                .map_err(|_| "Invalid MCP server metadata")?;
        if self.pages.len().saturating_add(data.len()) > MAX_MCP_SERVERS {
            return Err("MCP inventory contains too many configured servers".into());
        }
        for server in &mut data {
            validate_server(server)?;
        }
        for server in data {
            if server.name.is_empty() || self.pages.contains_key(&server.name) {
                return Err("MCP inventory contains an empty or repeated server name".into());
            }
            self.pages.insert(server.name.clone(), server);
        }
        if let Some(next) = next {
            if !self.cursors.insert(next.clone()) {
                return Err("MCP inventory returned a repeated or empty cursor".into());
            }
            return Ok(Some(
                self.begin(Operation::List, self.list_call(Some(&next))),
            ));
        }
        self.view.servers = std::mem::take(&mut self.pages).into_values().collect();
        self.cursors.clear();
        self.view.inventory_id = Some(uuid::Uuid::new_v4().to_string());
        self.view.current = true;
        Ok(None)
    }
    pub(super) fn reply(
        &mut self,
        id: &str,
        result: Result<Value, CallError>,
    ) -> Option<(String, Call)> {
        let (_, op) = self.pending.as_ref().filter(|(pending, _)| pending == id)?;
        let op = *op;
        self.pending = None;
        self.view.busy = false;
        let result = match result {
            Ok(value) => match op {
                Operation::ReadConfiguration => match Snapshot::read(&value) {
                    Ok(snapshot) => {
                        self.view.configuration = Some(ConfigurationView {
                            view_id: Uuid::new_v4().to_string(),
                            current: true,
                            snapshot,
                        });
                        Ok(None)
                    }
                    Err(error) => Err(error),
                },
                Operation::WriteConfiguration => {
                    let result = self
                        .write_target
                        .take()
                        .ok_or("Missing MCP write target".to_string())
                        .and_then(|target| mcp_configuration::saved(&value, &target));
                    match result {
                        Ok(overridden) => {
                            self.view.notice = Some(format!(
                                "MCP configuration saved in Codex's shared user file. {}Refresh configuration to inspect the result. Reload is a separate action and may start enabled servers; existing conversations were not reloaded.",
                                if overridden {
                                    "A higher-priority layer overrides this change. "
                                } else {
                                    ""
                                }
                            ));
                            Ok(None)
                        }
                        Err(error) => Err(error),
                    }
                }
                Operation::List => self.page(value),
                Operation::Reload => {
                    self.view.notice = Some("Native reload accepted. Refresh is queued for loaded threads; completion is not implied by this acknowledgement.".into());
                    Ok(Some(self.list()))
                }
                Operation::Login => {
                    if let Some(login) = &mut self.login {
                        login.acknowledged = true;
                        if login.completed.is_none() {
                            login.url = value
                                .get("authorizationUrl")
                                .and_then(Value::as_str)
                                .and_then(|value| oauth_url(value).ok());
                            if login.url.is_none() {
                                self.view.error = Some("Codex returned an unsupported authorization URL. No page was opened. The native sign-in may still be pending; reconnect before starting another attempt.".into());
                            }
                        }
                    }
                    self.sync_login();
                    Ok(None)
                }
                Operation::ReadResource => (|| -> Result<Option<(String, Call)>, String> {
                    let target = self
                        .direct_target
                        .take()
                        .ok_or_else(|| "Missing MCP resource request scope".to_owned())?;
                    let contents = parse_resource_result(&value, target.uri.as_deref())?;
                    self.view.direct_result = Some(DirectResult {
                        operation: "resource",
                        server: target.server,
                        name: target.name,
                        contents,
                        structured_content: None,
                        is_error: false,
                    });
                    Ok(None)
                })(),
                Operation::CallTool => (|| -> Result<Option<(String, Call)>, String> {
                    let target = self
                        .direct_target
                        .take()
                        .ok_or_else(|| "Missing MCP tool request scope".to_owned())?;
                    let (contents, structured_content, is_error) = parse_tool_result(&value)?;
                    self.view.direct_result = Some(DirectResult {
                        operation: "tool",
                        server: target.server,
                        name: target.name,
                        contents,
                        structured_content,
                        is_error,
                    });
                    Ok(None)
                })(),
            },
            Err(error) => {
                if matches!(
                    op,
                    Operation::ReadConfiguration | Operation::WriteConfiguration
                ) {
                    self.write_target = None;
                    self.stale_configuration();
                }
                if op == Operation::Login {
                    self.login = None;
                    self.sync_login();
                }
                if matches!(op, Operation::ReadResource | Operation::CallTool) {
                    self.direct_target = None;
                }
                Err(
                    if matches!(
                        op,
                        Operation::ReadConfiguration | Operation::WriteConfiguration
                    ) {
                        "Native MCP configuration request failed or its result is uncertain. Refresh before retrying; no automatic retry or reload was performed.".into()
                    } else if op == Operation::Login {
                        "Native MCP sign-in request failed. Check the server configuration before retrying.".into()
                    } else if op == Operation::CallTool {
                        "The direct MCP tool call failed or its result is uncertain. Inspect the external service before retrying; nothing was replayed automatically.".into()
                    } else if op == Operation::ReadResource {
                        "The native MCP resource read failed. Refresh the inventory before retrying; no tool was called.".into()
                    } else {
                        error.to_string()
                    },
                )
            }
        };
        match result {
            Ok(next) => next,
            Err(error) => {
                self.pages.clear();
                self.cursors.clear();
                self.view.error = Some(error);
                None
            }
        }
    }
    pub(super) fn completed(&mut self, params: &Value) {
        // A server name alone cannot identify the global or conversation flow.
        if params.get("threadId") != Some(&json!(self.view.thread_id)) {
            return;
        }
        let Some(login) = self.login.as_mut().filter(|login| {
            params.get("name").and_then(Value::as_str) == Some(login.name.as_str())
        }) else {
            return;
        };
        let Some(success) = params.get("success").and_then(Value::as_bool) else {
            return;
        };
        if login.completed.is_some() {
            return;
        }
        login.completed = Some(success);
        login.url = None;
        self.sync_login();
    }
    fn sync_login(&mut self) {
        if let Some(login) = &self.login
            && login.acknowledged
            && let Some(success) = login.completed
        {
            self.view.notice = success.then(|| "MCP sign-in completed. Refresh the configured-server list to read its current status.".into());
            self.view.error = (!success).then(|| {
                "MCP sign-in did not complete. Refresh the inventory before retrying.".into()
            });
            self.view.current = false;
            self.login = None;
        }
        self.view.login = self.login.as_ref().map(|login| LoginView {
            attempt_id: login.id.clone(),
            name: login.name.clone(),
            origin: login
                .url
                .as_ref()
                .map(|url| url.origin().ascii_serialization()),
            can_open: login.acknowledged && login.url.is_some() && login.completed.is_none(),
        });
    }
    pub(super) fn login_url(&self, id: &str) -> Result<&url::Url, String> {
        self.login
            .as_ref()
            .filter(|login| login.id == id && login.acknowledged && login.completed.is_none())
            .and_then(|login| login.url.as_ref())
            .ok_or_else(|| "This sign-in link is no longer active".into())
    }
    pub(super) fn disconnect(&mut self) {
        let was_writing = self.writing();
        let tool_uncertain = self.direct_tool_pending();
        self.write_target = None;
        self.direct_target = None;
        self.stale_configuration();
        self.pending = None;
        self.login = None;
        self.pages.clear();
        self.cursors.clear();
        self.view.current = false;
        self.view.busy = false;
        self.view.login = None;
        self.view.notice = Some(if was_writing {
            "Disconnected during an MCP configuration write. Its result is uncertain: refresh after reconnecting; nothing will be replayed."
        } else if tool_uncertain {
            "Disconnected during a direct MCP tool call. External side effects and the result are uncertain; inspect the service before retrying. Nothing will be replayed."
        } else {
            "Disconnected. Pending MCP requests were discarded, not replayed."
        }.into());
    }
    pub(super) fn invalidate(&mut self, notice: &str) {
        self.disconnect();
        self.view.notice = Some(notice.into());
    }
    pub(super) fn invalidate_inventory(&mut self, notice: &str) {
        self.view.current = false;
        self.view.direct_result = None;
        self.direct_target = None;
        self.pages.clear();
        self.cursors.clear();
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, op)| *op == Operation::List)
        {
            self.pending = None;
            self.view.busy = false;
        }
        // Startup/auth changes do not cancel an OAuth flow or its pending ACK.
        self.view.notice = Some(notice.into());
    }
}
fn can_login(server: &Server) -> bool {
    matches!(server.auth_status.as_str(), "notLoggedIn" | "oAuth")
        || server.runtime_status.as_deref() == Some("authenticationRequired")
}
fn oauth_url(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid authorization URL")?;
    let local = matches!(url.host(), Some(url::Host::Domain("localhost")))
        || match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "Authorization requires HTTPS or loopback HTTP without embedded credentials".into(),
        );
    }
    Ok(url)
}

fn validate_server(server: &mut Server) -> Result<(), String> {
    if !bounded_text(&server.name, 512)
        || server.tools.len() > 256
        || server.resources.len() > 512
        || server.resource_templates.len() > 512
    {
        return Err("MCP inventory contains invalid or excessive server metadata".into());
    }
    if server
        .runtime_status
        .as_deref()
        .is_some_and(|status| !bounded_text(status, 128))
        || !bounded_text(&server.auth_status, 128)
    {
        return Err("MCP inventory contains an invalid server status".into());
    }
    for (key, tool) in &mut server.tools {
        validate_entry(tool)?;
        if !bounded_text(key, 512) || key != &tool.name {
            return Err("MCP inventory contains a mismatched tool identity".into());
        }
        let schema = tool
            .raw_input_schema
            .as_ref()
            .ok_or("MCP tool inventory is missing its input schema")?;
        validate_json_value_with_limits(
            schema,
            0,
            &mut 0usize,
            MAX_MCP_SCHEMA_DEPTH,
            MAX_JSON_NODES,
        )?;
        tool.input_schema = public_tool_schema(schema)?;
    }
    for resource in &mut server.resources {
        validate_entry(resource)?;
        let uri = resource
            .uri
            .as_deref()
            .filter(|uri| bounded_text(uri, 16 * 1024))
            .ok_or("MCP resource inventory contains an invalid URI")?;
        // UI receives only an opaque inventory-bound handle. The URI remains
        // Rust-side because it can carry query material or provider identifiers.
        let _ = uri;
        resource.entry_id = Uuid::new_v4().to_string();
    }
    for template in &server.resource_templates {
        validate_entry(template)?;
    }
    Ok(())
}

fn validate_entry(entry: &Entry) -> Result<(), String> {
    if !bounded_text(&entry.name, 512)
        || entry
            .description
            .as_deref()
            .is_some_and(|description| !bounded_text(description, 8 * 1024))
    {
        return Err("MCP inventory contains invalid entry metadata".into());
    }
    Ok(())
}

fn validate_arguments(arguments: &Value) -> Result<(), String> {
    if !arguments.is_object() {
        return Err("Direct MCP tool arguments must be a JSON object".into());
    }
    validate_json_value(arguments, 0, &mut 0usize)?;
    let encoded = serde_json::to_vec(arguments).map_err(|_| "Invalid MCP tool arguments")?;
    if encoded.len() > 64 * 1024 {
        return Err("Direct MCP tool arguments exceed 64 KiB".into());
    }
    Ok(())
}

fn public_tool_schema(schema: &Value) -> Result<Option<String>, String> {
    let Some(object) = schema.as_object() else {
        return Ok(None);
    };
    if object.get("type").and_then(Value::as_str) != Some("object")
        && !object.get("properties").is_some_and(Value::is_object)
    {
        return Ok(None);
    }
    let sanitized = sanitize_schema(schema, 0)?;
    let rendered =
        serde_json::to_string_pretty(&sanitized).map_err(|_| "MCP tool input schema is invalid")?;
    if rendered.len() > 64 * 1024 {
        return Err("MCP tool input schema exceeds the display limit".into());
    }
    Ok(Some(rendered))
}

fn sanitize_schema(value: &Value, depth: usize) -> Result<Value, String> {
    if depth > MAX_MCP_SCHEMA_DEPTH {
        return Err("MCP tool input schema is too deeply nested".into());
    }
    let object = value
        .as_object()
        .ok_or("MCP tool input schema contains a non-object schema node")?;
    let mut output = serde_json::Map::new();
    for (key, value) in object {
        match key.as_str() {
            "type" | "title" | "description" | "format" => {
                let text = value
                    .as_str()
                    .filter(|text| bounded_text(text, 8 * 1024))
                    .ok_or("MCP tool input schema contains invalid text")?;
                output.insert(key.clone(), json!(text));
            }
            "required" => {
                let required = value
                    .as_array()
                    .filter(|items| items.len() <= 128)
                    .ok_or("MCP tool input schema contains invalid required fields")?;
                let fields = required
                    .iter()
                    .map(|field| {
                        field
                            .as_str()
                            .filter(|field| bounded_text(field, 512))
                            .map(str::to_owned)
                            .ok_or("MCP tool input schema contains an invalid required field")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                output.insert(key.clone(), json!(fields));
            }
            "properties" => {
                let properties = value
                    .as_object()
                    .filter(|properties| properties.len() <= 128)
                    .ok_or("MCP tool input schema contains invalid properties")?;
                let mut sanitized = serde_json::Map::new();
                for (name, property) in properties {
                    if !bounded_text(name, 512) {
                        return Err(
                            "MCP tool input schema contains an invalid property name".into()
                        );
                    }
                    sanitized.insert(name.clone(), sanitize_schema(property, depth + 1)?);
                }
                output.insert(key.clone(), Value::Object(sanitized));
            }
            "items" => {
                output.insert(key.clone(), sanitize_schema(value, depth + 1)?);
            }
            "enum" | "oneOf" | "anyOf" => {
                let values = value
                    .as_array()
                    .filter(|values| values.len() <= 64)
                    .ok_or("MCP tool input schema contains too many choices")?;
                let sanitized = if key == "enum" {
                    values.clone()
                } else {
                    values
                        .iter()
                        .map(|choice| sanitize_schema(choice, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?
                };
                output.insert(key.clone(), Value::Array(sanitized));
            }
            "default"
            | "const"
            | "minimum"
            | "maximum"
            | "minLength"
            | "maxLength"
            | "minItems"
            | "maxItems"
            | "additionalProperties" => {
                validate_json_value(value, depth + 1, &mut 0usize)?;
                output.insert(key.clone(), value.clone());
            }
            _ => {}
        }
    }
    Ok(Value::Object(output))
}

fn validate_json_value(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
    validate_json_value_with_limits(value, depth, nodes, MAX_JSON_DEPTH, MAX_JSON_NODES)
}

fn validate_json_value_with_limits(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
    maximum_depth: usize,
    maximum_nodes: usize,
) -> Result<(), String> {
    *nodes = nodes.saturating_add(1);
    if depth > maximum_depth || *nodes > maximum_nodes {
        return Err("JSON data exceeds the supported nesting or node limit".into());
    }
    match value {
        Value::String(text) if text.len() > 64 * 1024 || text.contains('\0') => {
            Err("JSON data contains an invalid or excessive string".into())
        }
        Value::Array(values) if values.len() > 256 => {
            Err("JSON data contains too many array entries".into())
        }
        Value::Array(values) => {
            for value in values {
                validate_json_value_with_limits(
                    value,
                    depth + 1,
                    nodes,
                    maximum_depth,
                    maximum_nodes,
                )?;
            }
            Ok(())
        }
        Value::Object(values) if values.len() > 256 => {
            Err("JSON data contains too many object fields".into())
        }
        Value::Object(values) => {
            for (key, value) in values {
                if !bounded_text(key, 512) {
                    return Err("JSON data contains an invalid object key".into());
                }
                validate_json_value_with_limits(
                    value,
                    depth + 1,
                    nodes,
                    maximum_depth,
                    maximum_nodes,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn parse_resource_result(
    value: &Value,
    expected_uri: Option<&str>,
) -> Result<Vec<DirectContent>, String> {
    let contents = value
        .get("contents")
        .and_then(Value::as_array)
        .ok_or("MCP resource response is incomplete")?;
    if contents.len() > 32 {
        return Err("MCP resource response contains too many content blocks".into());
    }
    let mut total = 0usize;
    contents
        .iter()
        .map(|content| {
            let uri = content
                .get("uri")
                .and_then(Value::as_str)
                .filter(|uri| bounded_text(uri, 16 * 1024))
                .ok_or("MCP resource response contains an invalid URI")?;
            if expected_uri != Some(uri) {
                return Err("MCP resource response changed the requested resource identity".into());
            }
            let mime_type = optional_bounded(content, "mimeType", 512)?;
            if let Some(text) = content.get("text").and_then(Value::as_str) {
                if text.len() > 256 * 1024 || text.contains('\0') {
                    return Err("MCP resource text exceeds the display limit".into());
                }
                total = total.saturating_add(text.len());
                if total > 512 * 1024 {
                    return Err("MCP resource response exceeds the combined display limit".into());
                }
                Ok(DirectContent {
                    kind: "text",
                    mime_type,
                    text: text.into(),
                })
            } else if let Some(blob) = content.get("blob").and_then(Value::as_str) {
                if blob.len() > 1024 * 1024 {
                    return Err("MCP resource binary content exceeds the inspection limit".into());
                }
                Ok(DirectContent {
                    kind: "binary",
                    mime_type,
                    text: format!("Binary content omitted ({} encoded bytes)", blob.len()),
                })
            } else {
                Err("MCP resource response contains an unsupported content block".into())
            }
        })
        .collect()
}

fn parse_tool_result(value: &Value) -> Result<(Vec<DirectContent>, Option<String>, bool), String> {
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or("MCP tool response is incomplete")?;
    if content.len() > 64 {
        return Err("MCP tool response contains too many content blocks".into());
    }
    let mut total = 0usize;
    let mut contents = Vec::with_capacity(content.len());
    for item in content {
        let kind = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let (public_kind, text, mime_type) = match kind {
            "text" => {
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| text.len() <= 256 * 1024 && !text.contains('\0'))
                    .ok_or("MCP tool text exceeds the display limit")?;
                ("text", text.to_owned(), None)
            }
            "resource" => {
                let resource = item
                    .get("resource")
                    .filter(|resource| resource.is_object())
                    .ok_or("MCP tool response contains an invalid embedded resource")?;
                if let Some(text) = resource.get("text").and_then(Value::as_str) {
                    if text.len() > 256 * 1024 || text.contains('\0') {
                        return Err("MCP embedded resource exceeds the display limit".into());
                    }
                    (
                        "resource",
                        text.to_owned(),
                        optional_bounded(resource, "mimeType", 512)?,
                    )
                } else {
                    (
                        "resource",
                        "Binary embedded resource omitted".into(),
                        optional_bounded(resource, "mimeType", 512)?,
                    )
                }
            }
            "image" | "audio" => (
                "media",
                format!("{} content omitted from direct inspection", kind),
                optional_bounded(item, "mimeType", 512)?,
            ),
            _ => (
                "unsupported",
                format!("Unsupported {kind} content omitted"),
                None,
            ),
        };
        total = total.saturating_add(text.len());
        if total > 512 * 1024 {
            return Err("MCP tool response exceeds the combined display limit".into());
        }
        contents.push(DirectContent {
            kind: public_kind,
            mime_type,
            text,
        });
    }
    let structured_content = match value.get("structuredContent") {
        None | Some(Value::Null) => None,
        Some(structured) => {
            validate_json_value(structured, 0, &mut 0usize)?;
            let rendered = serde_json::to_string_pretty(structured)
                .map_err(|_| "MCP structured content is invalid")?;
            if rendered.len() > 256 * 1024 {
                return Err("MCP structured content exceeds the display limit".into());
            }
            Some(rendered)
        }
    };
    let is_error = match value.get("isError") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err("MCP tool response contains an invalid error state".into()),
    };
    Ok((contents, structured_content, is_error))
}

fn optional_bounded(value: &Value, key: &str, maximum: usize) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if bounded_text(text, maximum) => Ok(Some(text.clone())),
        _ => Err(format!("MCP response contains an invalid {key}")),
    }
}

fn bounded_text(text: &str, maximum: usize) -> bool {
    !text.is_empty() && text.len() <= maximum && !text.contains('\0')
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_mcp(&mut self, action: McpAction) {
        let result = (|| {
            if !self.app_server.view.connected || self.app_server.client.is_none() {
                return Err("Connect Codex App Server first".into());
            }
            if let McpAction::OpenLogin { attempt_id } = action {
                let url = self.app_server.mcp.login_url(&attempt_id)?;
                return crate::external_browser::open(url.as_str());
            }
            if matches!(action, McpAction::InspectConfiguration {})
                && self.app_server.native_config_pending()
            {
                return Err("Wait for the pending native configuration write before inspecting MCP settings".into());
            }
            if matches!(
                action,
                McpAction::Reload { .. } | McpAction::ChangeConfiguration { .. }
            ) && (self.app_server.any_busy()
                || self.app_server.view.sandbox_setup.busy()
                || self
                    .agent_submission_queue
                    .iter()
                    .chain(&self.pending_agent_submissions)
                    .any(|input| input.provider == AgentProviderKind::CodexAppServer))
            {
                return Err(
                    "Finish active Codex work, configuration writes and sandbox setup before changing MCP configuration"
                        .into(),
                );
            }
            let (id, call) = self.app_server.mcp.action(action)?;
            if call.method == "config/batchWrite" {
                self.invalidate_app_server_preferences();
                self.invalidate_app_server_skills();
            }
            if call.method == "config/mcpServer/reload" {
                for owner in self.app_server.thread_mcp.invalidate_inventories("Native MCP reload requested. Refresh after the conversation's servers finish updating.") {
                    self.emit_app_server_conversation(&owner, "stream");
                }
            }
            self.app_server_mcp_call(id, call);
            Ok(())
        })();
        if let Err(error) = result {
            self.app_server.mcp.view.error = Some(error);
        }
        self.app_server.sync_profile();
        self.render_agent_panel();
    }
    pub(super) fn app_server_mcp_call(&self, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::McpReply {
                epoch,
                id,
                result,
            }));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured() -> McpState {
        let mut state = McpState::default();
        let (id, call) = state.action(McpAction::InspectConfiguration {}).unwrap();
        assert_eq!(call.method, "config/read");
        assert_eq!(call.params, json!({"includeLayers":true}));
        let file = std::env::current_dir().unwrap().join("config.toml");
        state.reply(&id,Ok(json!({"config":{"mcp_servers":{"fixture":{"command":"secret-command","enabled":false}}},"origins":{},"layers":[{"name":{"type":"user","file":file,"profile":null},"version":"fixture-version","disabledReason":null,"config":{"mcp_servers":{"fixture":{"command":"secret-command","enabled":false}}}}]})));
        assert!(state.view.configuration.as_ref().unwrap().current);
        state
    }
    #[test]
    fn mcp_configuration_writes_are_view_bound_and_never_reload_or_publish_values() {
        let mut state = configured();
        let view_id = state.view.configuration.as_ref().unwrap().view_id.clone();
        assert!(
            !serde_json::to_string(&state.view)
                .unwrap()
                .contains("secret-command")
        );
        assert!(
            state
                .action(McpAction::ChangeConfiguration {
                    view_id: "stale".into(),
                    edit: Edit::SetEnabled {
                        name: "fixture".into(),
                        enabled: true
                    }
                })
                .is_err()
        );
        let (id, call) = state
            .action(McpAction::ChangeConfiguration {
                view_id,
                edit: Edit::SetEnabled {
                    name: "fixture".into(),
                    enabled: true,
                },
            })
            .unwrap();
        assert_eq!(call.method, "config/batchWrite");
        assert_eq!(call.params["reloadUserConfig"], false);
        assert_eq!(call.params["expectedVersion"], "fixture-version");
        assert!(state.writing());
        assert!(!state.view.configuration.as_ref().unwrap().current);
        assert!(state.action(McpAction::InspectConfiguration {}).is_err());
        assert!(state.reply("foreign", Ok(json!({}))).is_none());
        assert!(state.writing());
        let target = state.write_target.as_ref().unwrap().file.clone();
        assert!(
            state
                .reply(
                    &id,
                    Ok(json!({"status":"ok","version":"v2","filePath":target}))
                )
                .is_none()
        );
        assert!(!state.writing());
        assert!(state.view.notice.as_ref().unwrap().contains("not reloaded"));
    }
    #[test]
    fn mcp_configuration_invalidation_discards_old_reads_but_preserves_write_receipts() {
        let mut state = configured();
        let (read_id, _) = state.action(McpAction::InspectConfiguration {}).unwrap();
        state.stale_configuration();
        assert!(!state.view.busy);
        assert!(!state.view.configuration.as_ref().unwrap().current);
        assert!(state.reply(&read_id, Ok(json!({}))).is_none());
        assert!(state.view.error.is_none());
        assert!(!state.view.configuration.as_ref().unwrap().current);

        let mut state = configured();
        let view_id = state.view.configuration.as_ref().unwrap().view_id.clone();
        let (write_id, _) = state
            .action(McpAction::ChangeConfiguration {
                view_id,
                edit: Edit::Remove {
                    name: "fixture".into(),
                },
            })
            .unwrap();
        state.stale_configuration();
        assert!(state.writing() && state.view.busy);
        assert_eq!(state.pending.as_ref().unwrap().0, write_id);
        assert!(state.write_target.is_some());
    }
    #[test]
    fn mcp_configuration_disconnect_invalidates_confirmation_and_ignores_late_ack() {
        let mut state = configured();
        let view_id = state.view.configuration.as_ref().unwrap().view_id.clone();
        let (id, _) = state
            .action(McpAction::ChangeConfiguration {
                view_id: view_id.clone(),
                edit: Edit::Remove {
                    name: "fixture".into(),
                },
            })
            .unwrap();
        state.disconnect();
        assert!(!state.writing());
        assert!(state.view.notice.as_ref().unwrap().contains("uncertain"));
        assert!(state.reply(&id, Ok(json!({"status":"ok"}))).is_none());
        assert!(
            state
                .action(McpAction::ChangeConfiguration {
                    view_id,
                    edit: Edit::Remove {
                        name: "fixture".into()
                    }
                })
                .is_err()
        );
        let mut scoped = McpState::for_thread("t");
        assert!(scoped.action(McpAction::InspectConfiguration {}).is_err());
        assert!(
            scoped
                .action(McpAction::ChangeConfiguration {
                    view_id: "v".into(),
                    edit: Edit::Remove {
                        name: "fixture".into()
                    }
                })
                .is_err()
        );
    }
    #[test]
    fn thread_inventory_cannot_reload_shared_configuration_and_login_is_scoped() {
        let mut state = McpState::for_thread("thread-a");
        let (id, _) = state.refresh().unwrap();
        state.reply(
            &id,
            Ok(json!({"data":[server("fixture")],"nextCursor":null})),
        );
        let inventory_id = state.view.inventory_id.clone().unwrap();
        assert!(
            state
                .action(McpAction::Reload {
                    inventory_id: inventory_id.clone()
                })
                .is_err()
        );
        assert!(state.view.current && state.view.login.is_none());
        let (_, call) = state
            .action(McpAction::Login {
                inventory_id,
                name: "fixture".into(),
            })
            .unwrap();
        assert_eq!(call.method, "mcpServer/oauth/login");
        assert_eq!(call.params, json!({"name":"fixture","threadId":"thread-a"}));
    }
    #[test]
    fn generated_contract_samples_feed_inventory_and_oauth() {
        let samples: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-mcp.json"
        )))
        .unwrap();
        let mut state = McpState::default();
        let (id, _) = state.action(McpAction::Refresh {}).unwrap();
        let (next, _) = state.reply(&id, Ok(samples[0]["params"].clone())).unwrap();
        state.reply(&next, Ok(samples[1]["params"].clone()));
        assert_eq!(state.view.servers[0].resources[0].name, "Overview");
        let id = login(&mut state);
        state.completed(&samples[4]["params"]);
        assert!(state.login.as_ref().unwrap().completed.is_none());
        state.reply(&id, Ok(samples[2]["params"].clone()));
        state.completed(&samples[3]["params"]);
        assert!(state.view.login.is_none());
        assert!(state.view.notice.as_ref().unwrap().contains("completed"));
    }
    fn server(name: &str) -> Value {
        json!({"name":name,"runtimeStatus":null,"authStatus":"notLoggedIn","tools":{"lookup":{"name":"lookup","inputSchema":{"secret":"not-published"}}},"resources":[],"resourceTemplates":[]})
    }
    fn nested_object_schema(levels: usize) -> Value {
        let mut schema = json!({"type":"string"});
        for _ in 0..levels {
            schema = json!({"type":"object","properties":{"value":schema}});
        }
        schema
    }
    fn ready() -> McpState {
        let mut state = McpState::default();
        let (id, _) = state.action(McpAction::Refresh {}).unwrap();
        state.reply(
            &id,
            Ok(json!({"data":[server("fixture")],"nextCursor":null})),
        );
        state
    }
    fn login(state: &mut McpState) -> String {
        state
            .action(McpAction::Login {
                inventory_id: state.view.inventory_id.clone().unwrap(),
                name: "fixture".into(),
            })
            .unwrap()
            .0
    }
    #[test]
    fn inventory_is_atomic_paginated_and_redacted() {
        let mut s = ready();
        let previous = s.view.inventory_id.clone();
        let (id, _) = s.action(McpAction::Refresh {}).unwrap();
        let (next, call) = s
            .reply(
                &id,
                Ok(json!({"data":[server("two")],"nextCursor":"page2"})),
            )
            .unwrap();
        assert_eq!(call.params["cursor"], "page2");
        assert!(!s.view.current);
        assert_eq!(s.view.servers[0].name, "fixture");
        assert!(s.reply(&id, Ok(json!({}))).is_none());
        assert!(s.view.busy);
        s.reply(
            &next,
            Ok(json!({"data":[server("three")],"nextCursor":null})),
        );
        assert_eq!(s.view.servers.len(), 2);
        assert!(s.view.current);
        assert_ne!(previous, s.view.inventory_id);
        let view = serde_json::to_string(&s.view).unwrap();
        assert!(!view.contains("not-published"));
        assert!(view.contains("\"runtimeStatus\":null"));
    }
    #[test]
    fn generated_nested_tool_schemas_use_the_separate_read_only_budget() {
        let schema = nested_object_schema(5);
        assert!(validate_json_value(&schema, 0, &mut 0).is_err());
        let mut state = McpState::default();
        let (id, _) = state.action(McpAction::Refresh {}).unwrap();
        state.reply(
            &id,
            Ok(json!({
                "data":[{
                    "name":"fixture","runtimeStatus":"connected","authStatus":"unsupported",
                    "tools":{"deep":{"name":"deep","inputSchema":schema}},
                    "resources":[],"resourceTemplates":[]
                }],
                "nextCursor":null
            })),
        );
        assert!(state.view.current);
        assert!(state.view.error.is_none());
        assert!(state.view.servers[0].tools["deep"].input_schema.is_some());
    }
    #[test]
    fn invalid_pages_and_stale_inventory_never_authorize_login() {
        let mut s = ready();
        assert!(
            s.action(McpAction::Login {
                inventory_id: "stale".into(),
                name: "fixture".into()
            })
            .is_err()
        );
        let (id, _) = s.action(McpAction::Refresh {}).unwrap();
        let (next, _) = s
            .reply(&id, Ok(json!({"data":[],"nextCursor":"same"})))
            .unwrap();
        s.reply(&next, Ok(json!({"data":[],"nextCursor":"same"})));
        assert!(s.view.error.is_some());
        assert!(!s.view.current);
        assert_eq!(s.view.servers.len(), 1);
    }
    #[test]
    fn oauth_races_scope_and_disconnect_do_not_leak_or_replay() {
        for early in [true, false] {
            let mut s = ready();
            let id = login(&mut s);
            let completed = json!({"name":"fixture","threadId":null,"success":true});
            s.completed(&json!({"name":"fixture","threadId":"foreign","success":true}));
            assert!(s.login.as_ref().unwrap().completed.is_none());
            if early {
                s.completed(&completed);
            }
            s.reply(
                &id,
                Ok(json!({"authorizationUrl":"https://example.test/authorize?state=private"})),
            );
            if !early {
                assert!(s.login_url(&id).is_ok());
                assert!(!serde_json::to_string(&s.view).unwrap().contains("private"));
                s.completed(&completed);
            }
            assert!(s.login.is_none());
            assert!(s.login_url(&id).is_err());
            assert!(!s.view.current);
        }
        let mut s = ready();
        let id = login(&mut s);
        s.disconnect();
        s.reply(&id, Ok(json!({"authorizationUrl":"https://example.test"})));
        assert!(s.view.login.is_none());
        assert!(!s.view.current);
    }
    #[test]
    fn unsafe_url_never_opens_and_unknown_native_flow_stays_pending() {
        for bad in [
            "file:///c:/a",
            "javascript:alert(1)",
            "http://remote.test/auth",
            "https://user:pass@example.test",
        ] {
            assert!(oauth_url(bad).is_err());
        }
        for good in [
            "https://example.test/auth",
            "http://127.0.0.1:3142/auth",
            "http://[::1]:3142/auth",
        ] {
            assert!(oauth_url(good).is_ok());
        }
        let mut s = ready();
        let id = login(&mut s);
        s.reply(&id, Ok(json!({"authorizationUrl":"file:///c:/a"})));
        assert!(s.login_url(&id).is_err());
        assert!(s.view.login.is_some());
        assert!(s.action(McpAction::Refresh {}).is_err());
    }
    #[test]
    fn reload_only_uses_native_api_and_ack_is_not_completion() {
        let mut s = ready();
        let (id, call) = s
            .action(McpAction::Reload {
                inventory_id: s.view.inventory_id.clone().unwrap(),
            })
            .unwrap();
        assert_eq!(call.method, "config/mcpServer/reload");
        assert!(call.params.is_null());
        let (_, call) = s.reply(&id, Ok(json!({}))).unwrap();
        assert_eq!(call.method, "mcpServerStatus/list");
        assert!(!s.view.current);
        assert!(s.view.notice.as_ref().unwrap().contains("queued"));
    }

    fn direct_ready() -> McpState {
        let mut state = McpState::for_thread("thread-a");
        let (id, _) = state.refresh().unwrap();
        state.reply(
            &id,
            Ok(json!({"data":[{
                "name":"fixture","runtimeStatus":"connected","authStatus":"oAuth",
                "tools":{"lookup":{"name":"lookup","description":"Search records","inputSchema":{
                    "type":"object","properties":{"query":{"type":"string","description":"Term"}},
                    "required":["query"],"privateExtension":"DROP_SCHEMA"
                }}},
                "resources":[{"name":"Overview","description":"Public summary","uri":"fixture://private/resource?token=DROP_URI"}],
                "resourceTemplates":[]
            }],"nextCursor":null})),
        );
        assert!(state.view.current);
        state
    }

    #[test]
    fn direct_resources_use_opaque_handles_and_correlate_the_returned_uri() {
        let mut state = direct_ready();
        let public_value = serde_json::to_value(&state.view).unwrap();
        let public = serde_json::to_string(&public_value).unwrap();
        let resource = &public_value["servers"][0]["resources"][0];
        let tool = &public_value["servers"][0]["tools"]["lookup"];
        assert!(resource["entryId"].is_string());
        assert!(resource.get("entry_id").is_none());
        assert!(tool["inputSchema"].is_string());
        assert!(tool.get("input_schema").is_none());
        assert!(!public.contains("DROP_URI"));
        assert!(!public.contains("DROP_SCHEMA"));
        assert!(public.contains("query"));
        let inventory = state.view.inventory_id.clone().unwrap();
        let resource_id = state.view.servers[0].resources[0].entry_id.clone();
        assert!(!resource_id.is_empty());
        let (id, call) = state
            .action(McpAction::ReadResource {
                inventory_id: inventory.clone(),
                server: "fixture".into(),
                resource_id: resource_id.clone(),
            })
            .unwrap();
        assert_eq!(call.method, "mcpServer/resource/read");
        assert_eq!(call.params["threadId"], "thread-a");
        assert_eq!(
            call.params["uri"],
            "fixture://private/resource?token=DROP_URI"
        );
        state.reply(
            &id,
            Ok(json!({"contents":[{"uri":"fixture://foreign","text":"wrong"}]})),
        );
        assert!(state.view.direct_result.is_none());
        assert!(state.view.error.is_some());

        let (id, _) = state
            .action(McpAction::ReadResource {
                inventory_id: inventory,
                server: "fixture".into(),
                resource_id,
            })
            .unwrap();
        state.reply(
            &id,
            Ok(json!({"contents":[{
                "uri":"fixture://private/resource?token=DROP_URI",
                "mimeType":"text/plain","text":"bounded result","_meta":{"secret":"DROP_META"}
            }]})),
        );
        let public = serde_json::to_string(&state.view).unwrap();
        assert!(public.contains("bounded result"));
        assert!(!public.contains("DROP_URI"));
        assert!(!public.contains("DROP_META"));
    }

    #[test]
    fn direct_tools_require_thread_inventory_object_arguments_and_no_replay() {
        let mut state = direct_ready();
        let inventory = state.view.inventory_id.clone().unwrap();
        assert!(
            state
                .action(McpAction::CallTool {
                    inventory_id: "stale".into(),
                    server: "fixture".into(),
                    tool: "lookup".into(),
                    arguments: json!({"query":"x"}),
                })
                .is_err()
        );
        assert!(
            state
                .action(McpAction::CallTool {
                    inventory_id: inventory.clone(),
                    server: "fixture".into(),
                    tool: "lookup".into(),
                    arguments: json!(["not-an-object"]),
                })
                .is_err()
        );
        let (id, call) = state
            .action(McpAction::CallTool {
                inventory_id: inventory,
                server: "fixture".into(),
                tool: "lookup".into(),
                arguments: json!({"query":"exact"}),
            })
            .unwrap();
        assert!(state.direct_tool_pending());
        assert_eq!(call.method, "mcpServer/tool/call");
        assert_eq!(call.params["arguments"], json!({"query":"exact"}));
        state.reply(
            &id,
            Ok(json!({
                "content":[{"type":"text","text":"tool result","annotations":{"secret":"DROP"}}],
                "structuredContent":{"count":1},"isError":false,"_meta":{"secret":"DROP"}
            })),
        );
        let public = serde_json::to_string(&state.view).unwrap();
        assert!(public.contains("tool result"));
        assert!(public.contains("count"));
        assert!(!public.contains("DROP"));

        let mut uncertain = direct_ready();
        let (_, _) = uncertain
            .action(McpAction::CallTool {
                inventory_id: uncertain.view.inventory_id.clone().unwrap(),
                server: "fixture".into(),
                tool: "lookup".into(),
                arguments: json!({}),
            })
            .unwrap();
        uncertain.disconnect();
        assert!(
            uncertain
                .view
                .notice
                .as_deref()
                .unwrap()
                .contains("side effects")
        );
        assert!(!uncertain.view.current);
        assert!(!uncertain.direct_tool_pending());
    }
}
