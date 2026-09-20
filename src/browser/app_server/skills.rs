//! Native skill discovery and explicit native configuration writes. Never reads
//! SKILL.md contents, expands instructions, downloads icons or runs a skill.
use super::*;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SkillsAction {
    Selection {},
    Reconnect {},
    Attach {
        view_id: String,
        path: String,
    },
    AttachOfficialBrowser {},
    Remove {
        id: String,
    },
    Refresh {
        request_id: String,
        expected_directory: String,
    },
    SetEnabled {
        view_id: String,
        path: String,
        enabled: bool,
    },
    SetExtraRoots {
        roots: Vec<String>,
    },
    Close {
        request_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SelectedSkill {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(serialize_with = "serialize_directory")]
    pub directory: PathBuf,
    pub current: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Skill {
    name: String,
    description: String,
    path: String,
    scope: String,
    enabled: bool,
    plugin_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct SkillError {
    path: String,
    message: String,
}

#[derive(Deserialize)]
struct Listing {
    cwd: String,
    skills: Vec<Skill>,
    errors: Vec<SkillError>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Inventory {
    request_id: String,
    view_id: String,
    #[serde(serialize_with = "serialize_directory")]
    directory: PathBuf,
    items: Vec<Skill>,
    errors: Vec<SkillError>,
    loading: bool,
    current: bool,
    error: Option<String>,
    #[serde(skip)]
    pending: Option<String>,
}

fn serialize_directory<S>(directory: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&display_path(directory))
}

struct Write {
    id: String,
    owner: String,
    path: String,
    enabled: bool,
}

#[derive(Default)]
pub(super) struct Skills {
    views: BTreeMap<String, Inventory>,
    selected: BTreeMap<String, Vec<SelectedSkill>>,
    observers: HashSet<String>,
    write: Option<Write>,
    extra_roots_write: Option<(String, String, Vec<PathBuf>)>,
    extra_roots: Vec<PathBuf>,
    pub(super) official_root: Option<PathBuf>,
    pub(super) browser_root: Option<PathBuf>,
    automatic_enabled: BTreeMap<PathBuf, bool>,
    pub(super) official_bootstrap_pending: bool,
    notice: Option<String>,
}
impl Skills {
    pub(super) fn set_computer_use(&mut self, attachment: Option<&computer_use::Attachment>) {
        self.official_root = attachment.and_then(|attachment| attachment.root.clone());
        self.browser_root = attachment.and_then(|attachment| attachment.browser_root.clone());
        self.automatic_enabled.clear();
        if let Some(attachment) = attachment {
            if let Some(root) = &attachment.root {
                self.automatic_enabled
                    .insert(root.join("computer-use/SKILL.md"), attachment.enabled);
            }
            if let Some(root) = &attachment.browser_root {
                self.automatic_enabled.insert(
                    root.join("control-in-app-browser/SKILL.md"),
                    attachment.browser_enabled,
                );
            }
        }
    }

    fn attach_official_browser(&mut self, owner: &str, directory: &Path) -> Result<(), String> {
        if self.writing() {
            return Err("Wait for the shared skill configuration write".into());
        }
        let root = self
            .browser_root
            .as_ref()
            .ok_or("The official Browser plugin is unavailable. Enable it in Codex desktop, then reconnect Supervisor")?;
        let path = root.join("control-in-app-browser/SKILL.md");
        if !path.is_file() || !self.automatic_allowed(owner, &path) {
            return Err("The official Browser skill is disabled or unavailable in Codex".into());
        }
        let path = display_path(&path);
        let selected = self.selected.entry(owner.into()).or_default();
        if !selected
            .iter()
            .any(|skill| skill.path == path && same_directory(&skill.directory, directory))
        {
            selected.push(SelectedSkill {
                id: Uuid::new_v4().to_string(),
                name: "browser:control-in-app-browser".into(),
                path,
                directory: directory.into(),
                current: true,
            });
        }
        Ok(())
    }

    fn automatic_allowed(&self, owner: &str, path: &Path) -> bool {
        if let Some(skill) = self
            .views
            .get(owner)
            .filter(|view| view.current)
            .and_then(|view| {
                view.items
                    .iter()
                    .find(|skill| same_directory(Path::new(&skill.path), path))
            })
        {
            return skill.enabled;
        }
        !self
            .automatic_enabled
            .iter()
            .any(|(candidate, enabled)| !enabled && same_directory(candidate, path))
    }

    fn attach(
        &mut self,
        owner: &str,
        view_id: &str,
        path: &str,
        directory: &Path,
    ) -> Result<(), String> {
        if self.writing() {
            return Err("Wait for the shared skill configuration write".into());
        }
        let view = self
            .views
            .get(owner)
            .filter(|v| v.current && !v.loading && v.view_id == view_id && v.directory == directory)
            .ok_or("Refresh this project's skills before attaching one")?;
        let skill = view
            .items
            .iter()
            .find(|s| s.path == path && s.enabled)
            .ok_or("Select an enabled skill from the native inventory")?;
        let selected = self.selected.entry(owner.into()).or_default();
        if skill.name.chars().any(char::is_whitespace) || skill.name.contains('\0') {
            return Err(
                "This native skill name cannot be used as an explicit skill reference".into(),
            );
        }
        if !selected
            .iter()
            .any(|s| s.path == path && s.directory == directory)
        {
            selected.push(SelectedSkill {
                id: Uuid::new_v4().to_string(),
                name: skill.name.clone(),
                path: skill.path.clone(),
                directory: directory.into(),
                current: true,
            });
        }
        Ok(())
    }
    fn remove(&mut self, owner: &str, id: &str) {
        if let Some(selected) = self.selected.get_mut(owner) {
            selected.retain(|s| s.id != id);
        }
    }
    pub(in crate::browser) fn capture(
        &self,
        owner: &str,
        directory: Option<&Path>,
    ) -> Result<Vec<SelectedSkill>, String> {
        let selected = self.selected.get(owner).cloned().unwrap_or_default();
        if selected
            .iter()
            .any(|s| !s.current || directory.is_none_or(|d| !same_directory(&s.directory, d)))
        {
            return Err("A selected Codex skill is stale or belongs to a different directory. Refresh Codex skills or remove it before sending.".into());
        }
        Ok(selected)
    }
    pub(in crate::browser) fn promote(&mut self, from: &str, to: &str) {
        if from != to
            && let Some(selected) = self.selected.remove(from)
        {
            self.selected.entry(to.into()).or_default().extend(selected);
        }
    }
    pub(in crate::browser) fn consume(&mut self, owner: &str, accepted: &[SelectedSkill]) {
        if let Some(selected) = self.selected.get_mut(owner) {
            selected.retain(|s| !accepted.iter().any(|a| a.id == s.id));
        }
    }
    pub(super) fn writing(&self) -> bool {
        self.write.is_some() || self.extra_roots_write.is_some() || self.official_bootstrap_pending
    }
    fn refresh(
        &mut self,
        owner: &str,
        request_id: String,
        directory: PathBuf,
    ) -> Result<(String, Call), String> {
        if self.writing() {
            return Err(
                "Wait for the native skill configuration response before refreshing".into(),
            );
        }
        if request_id.is_empty() {
            return Err("Skill discovery requires a view request ID".into());
        }
        let id = Uuid::new_v4().to_string();
        let call = api::skills_list(
            directory
                .to_str()
                .ok_or("The skill directory must be UTF-8")?,
        );
        self.views.insert(
            owner.into(),
            Inventory {
                request_id,
                view_id: Uuid::new_v4().to_string(),
                directory,
                items: vec![],
                errors: vec![],
                loading: true,
                current: false,
                error: None,
                pending: Some(id.clone()),
            },
        );
        Ok((id, call))
    }
    fn choose(
        &mut self,
        owner: &str,
        view_id: &str,
        path: &str,
        enabled: bool,
        directory: &Path,
    ) -> Result<(String, Call), String> {
        if self.writing() {
            return Err("A native skill configuration change is already pending".into());
        }
        let view = self
            .views
            .get(owner)
            .filter(|v| v.view_id == view_id && v.directory == directory && v.current && !v.loading)
            .ok_or("Refresh skills for this project before changing their configuration")?;
        let skill = view
            .items
            .iter()
            .find(|s| s.path == path)
            .ok_or("Select a skill from the current native inventory")?;
        if skill.enabled == enabled {
            return Err(
                "The selected skill already has this state; refresh its native configuration"
                    .into(),
            );
        }
        let id = Uuid::new_v4().to_string();
        self.write = Some(Write {
            id: id.clone(),
            owner: owner.into(),
            path: path.into(),
            enabled,
        });
        self.notice = None;
        self.invalidate();
        Ok((id, api::skill_enabled(path, enabled)))
    }
    fn set_extra_roots(
        &mut self,
        owner: &str,
        roots: Vec<String>,
    ) -> Result<(String, Call), String> {
        if self.writing() {
            return Err("A native skill configuration change is already pending".into());
        }
        if roots.len() > 16 {
            return Err("Select at most 16 extra skill roots".into());
        }
        let mut canonical = Vec::with_capacity(roots.len());
        let mut seen = HashSet::new();
        for root in roots {
            if root.is_empty() || root.len() > 16 * 1024 || root.contains('\0') {
                return Err("An extra skill root is invalid".into());
            }
            let path = PathBuf::from(&root);
            if !path.is_absolute() || !path.is_dir() {
                return Err(format!(
                    "Extra skill root is not an existing absolute directory: {root}"
                ));
            }
            let path = fs::canonicalize(&path)
                .map_err(|error| format!("Extra skill root could not be resolved: {error}"))?;
            if path.to_str().is_none() {
                return Err("Extra skill roots must be representable as UTF-8".into());
            }
            if crate::sensitive_path::is_sensitive_path(&path) {
                return Err(
                    "Credential and secret directories cannot be used as extra skill roots".into(),
                );
            }
            if seen.insert(path.clone()) {
                canonical.push(path);
            }
        }
        let mut effective = canonical.clone();
        if let Some(root) = &self.official_root
            && !effective.contains(root)
        {
            effective.push(root.clone());
        }
        if let Some(root) = &self.browser_root
            && !effective.contains(root)
        {
            effective.push(root.clone());
        }
        let wire = effective
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let id = Uuid::new_v4().to_string();
        self.extra_roots_write = Some((id.clone(), owner.into(), canonical));
        self.notice = None;
        self.invalidate();
        Ok((id, api::skills_extra_roots(&wire)))
    }
    fn close(&mut self, owner: &str, request_id: &str) {
        if self
            .views
            .get(owner)
            .is_some_and(|v| v.request_id == request_id)
        {
            self.views.remove(owner);
        }
        // Closing a view cannot cancel, forget or replay an already-sent write.
    }
    pub(super) fn invalidate(&mut self) {
        for skill in self.selected.values_mut().flatten() {
            skill.current = false;
        }
        for view in self.views.values_mut() {
            view.current = false;
            view.loading = false;
            view.pending = None;
        }
    }
    fn reply(&mut self, owner: &str, id: &str, result: Result<Value, CallError>) {
        if self
            .extra_roots_write
            .as_ref()
            .is_some_and(|(pending, pending_owner, _)| pending == id && pending_owner == owner)
        {
            let (_, _, roots) = self.extra_roots_write.take().unwrap();
            self.invalidate();
            match result {
                Ok(value) if value.as_object().is_some_and(|value| value.is_empty()) => {
                    self.extra_roots = roots;
                    self.notice = Some("Codex accepted the process-scoped extra skill roots. Refresh each project inventory to inspect effective discovery. Restarting or reconnecting clears this process setting.".into());
                }
                Ok(_) => self.notice = Some("Codex returned an unexpected extra-roots acknowledgement. Reconnect before retrying; nothing will be sent automatically.".into()),
                Err(error) => self.notice = Some(format!("Extra skill roots were not accepted: {error}. Reconnect or review the paths before retrying.")),
            }
            return;
        }
        if self
            .write
            .as_ref()
            .is_some_and(|w| w.id == id && w.owner == owner)
        {
            let write = self.write.take().unwrap();
            // An uncertain write cannot silently re-enable automatic attachment.
            // A subsequent native acknowledgement or inventory resolves it.
            self.automatic_enabled
                .retain(|path, _| !same_directory(path, Path::new(&write.path)));
            self.automatic_enabled.insert(
                PathBuf::from(&write.path),
                result
                    .as_ref()
                    .ok()
                    .and_then(|value| value["effectiveEnabled"].as_bool())
                    .unwrap_or(false),
            );
            self.invalidate();
            self.notice = Some(match result {
                Ok(value) => match value["effectiveEnabled"].as_bool() {
                    Some(actual) => format!("Codex reports {} for {}. {}Refresh the inventory to confirm the current project view.", if actual {"enabled"} else {"disabled"}, write.path, if actual != write.enabled {"The effective state differs from your request. "} else {""}),
                    None => "Codex returned no effective skill state. The write may have completed; refresh before deciding what to do next. Nothing will be resent automatically.".into(),
                },
                Err(error) => format!("Skill configuration result: {error}. Refresh to inspect the effective state; no write is automatically retried."),
            });
            return;
        }
        let Some(view) = self
            .views
            .get_mut(owner)
            .filter(|v| v.pending.as_deref() == Some(id))
        else {
            return;
        };
        view.pending = None;
        view.loading = false;
        let parsed = result.map_err(|e| e.to_string()).and_then(|value| {
            let mut entries: Vec<Listing> = serde_json::from_value(value["data"].clone())
                .map_err(|_| "Invalid native skills response")?;
            if entries.len() != 1 || Path::new(&entries[0].cwd) != view.directory {
                return Err("Native skills returned a different project directory".into());
            }
            let entry = entries.remove(0);
            let mut paths = HashSet::new();
            if entry.skills.iter().any(|s| {
                s.name.is_empty()
                    || !Path::new(&s.path).is_absolute()
                    || !paths.insert(&s.path)
                    || !matches!(s.scope.as_str(), "user" | "repo" | "system" | "admin")
            }) {
                return Err("Native skills returned invalid or duplicate identities".into());
            }
            Ok(entry)
        });
        match parsed {
            Ok(entry) => {
                if let Some(selected) = self.selected.get_mut(owner) {
                    for skill in selected {
                        skill.current = skill.directory == view.directory
                            && entry
                                .skills
                                .iter()
                                .any(|s| s.enabled && s.path == skill.path && s.name == skill.name);
                    }
                }
                view.items = entry.skills;
                view.errors = entry.errors;
                view.current = true;
                view.error = None;
            }
            Err(error) => {
                view.error = Some(error);
                view.current = false;
            }
        }
    }
    pub(super) fn disconnect(&mut self) {
        self.invalidate();
        self.official_bootstrap_pending = false;
        let uncertain = self.write.take().is_some() || self.extra_roots_write.take().is_some();
        self.extra_roots.clear();
        if uncertain {
            self.notice=Some("Codex disconnected during a skill configuration change. Its result is unknown. Reconnect and refresh; nothing will be replayed.".into());
        }
    }
}

impl BrowserApp {
    fn can_reconnect_computer_use(&self) -> bool {
        self.app_server.view.connected
            && !self.app_server.view.connecting
            && !self.app_server.view.auth_busy
            && self.app_server.view.login.is_none()
            && !self.app_server.any_busy()
            && !self.app_server.native_config_pending()
            && !self.app_server.view.sandbox_setup.busy()
            && !self
                .agent_submission_queue
                .iter()
                .chain(&self.pending_agent_submissions)
                .any(|input| input.provider == AgentProviderKind::CodexAppServer)
    }

    pub(in crate::browser) fn app_server_skills(&mut self, owner: String, action: SkillsAction) {
        let shared_change = matches!(
            &action,
            SkillsAction::SetEnabled { .. } | SkillsAction::SetExtraRoots { .. }
        );
        let result = (|| {
            if let SkillsAction::Selection {} = &action {
                self.app_server.skills.observers.insert(owner.clone());
                return Ok(());
            }
            if let SkillsAction::Remove { id } = &action {
                self.app_server.skills.remove(&owner, id);
                return Ok(());
            }
            if let SkillsAction::Close { request_id } = &action {
                self.app_server.skills.close(&owner, request_id);
                return Ok(());
            }
            if !self.app_server.view.connected || self.app_server.client.is_none() {
                return Err("Connect Codex App Server first".into());
            }
            if !self.native_access_selected(&owner) {
                return Err("Select Codex in this conversation first".into());
            }
            if matches!(action, SkillsAction::Reconnect {}) {
                if !self.can_reconnect_computer_use() {
                    return Err("Finish active or queued Codex work and pending settings before reconnecting. No desktop action has been repeated.".into());
                }
                self.app_server_account(AccountAction::Connect);
                return Ok(());
            }
            let target = self.conversation_target(&owner)?;
            let root=target.root.filter(|r|!target.remote && r.is_absolute() && r.is_dir()).ok_or("Select an existing local project or graph directory to discover its Codex skills")?;
            let (id, call) = match action {
                SkillsAction::AttachOfficialBrowser {} => {
                    if self.app_server.native_config_pending() {
                        return Err("Wait for the shared Codex configuration write".into());
                    }
                    self.app_server
                        .skills
                        .attach_official_browser(&owner, &root)?;
                    return Ok(());
                }
                SkillsAction::Attach { view_id, path } => {
                    if self.app_server.native_config_pending() {
                        return Err("Wait for the shared Codex configuration write".into());
                    }
                    self.app_server
                        .skills
                        .attach(&owner, &view_id, &path, &root)?;
                    return Ok(());
                }
                SkillsAction::Refresh {
                    request_id,
                    expected_directory,
                } => {
                    if !same_directory(Path::new(&expected_directory), &root) {
                        return Err("The project changed. Reopen its skill controls".into());
                    }
                    if self.app_server.native_config_pending() {
                        return Err("Wait for the shared Codex configuration write before refreshing skills".into());
                    }
                    self.app_server.skills.refresh(&owner, request_id, root)?
                }
                SkillsAction::SetEnabled {
                    view_id,
                    path,
                    enabled,
                } => {
                    if self.app_server.any_busy()
                        || self.app_server.view.sandbox_setup.busy()
                        || self
                            .agent_submission_queue
                            .iter()
                            .chain(&self.pending_agent_submissions)
                            .any(|input| input.provider == AgentProviderKind::CodexAppServer)
                    {
                        return Err("Finish active or queued Codex work and sandbox setup before changing shared skill configuration".into());
                    }
                    let prepared = self
                        .app_server
                        .skills
                        .choose(&owner, &view_id, &path, enabled, &root)?;
                    self.invalidate_app_server_preferences();
                    prepared
                }
                SkillsAction::SetExtraRoots { roots } => {
                    if self.app_server.any_busy()
                        || self.app_server.view.sandbox_setup.busy()
                        || self
                            .agent_submission_queue
                            .iter()
                            .chain(&self.pending_agent_submissions)
                            .any(|input| input.provider == AgentProviderKind::CodexAppServer)
                    {
                        return Err("Finish active or queued Codex work before changing process-scoped skill roots".into());
                    }
                    self.app_server.skills.set_extra_roots(&owner, roots)?
                }
                SkillsAction::Close { .. }
                | SkillsAction::Reconnect {}
                | SkillsAction::Selection {}
                | SkillsAction::Remove { .. } => unreachable!(),
            };
            let client = self.app_server.client.clone().unwrap();
            let proxy = self.proxy.clone();
            let epoch = self.app_server.epoch;
            let reply_owner = owner.clone();
            std::thread::spawn(move || {
                let result = call.send(&client).and_then(|ticket| ticket.wait());
                let _ = proxy.send_event(BrowserEvent::AppServer(Event::SkillsReply {
                    epoch,
                    owner: reply_owner,
                    id,
                    result,
                }));
            });
            Ok(())
        })();
        self.emit_app_server_skills(&owner, result.err());
        if shared_change {
            self.emit_other_skill_views(&owner);
        }
    }
    pub(super) fn app_server_skills_reply(
        &mut self,
        owner: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        let was_writing = self.app_server.skills.writing();
        self.app_server.skills.reply(owner, id, result);
        if was_writing && !self.app_server.skills.writing() {
            self.invalidate_app_server_preferences();
        }
        self.emit_app_server_skills(owner, None);
        if was_writing {
            self.emit_other_skill_views(owner);
        }
    }
    pub(super) fn invalidate_app_server_skills(&mut self) {
        self.app_server.mcp.stale_configuration();
        self.app_server.skills.invalidate();
        self.emit_other_skill_views("");
    }
    pub(super) fn emit_other_skill_views(&self, excluding: &str) {
        let mut emitted = HashSet::new();
        for owner in self
            .app_server
            .skills
            .views
            .keys()
            .chain(self.app_server.skills.selected.keys())
            .chain(self.app_server.skills.observers.iter())
            .filter(|o| o.as_str() != excluding)
        {
            if emitted.insert(owner) {
                self.emit_app_server_skills(owner, None);
            }
        }
    }
    pub(in crate::browser) fn emit_app_server_skills(&self, owner: &str, error: Option<String>) {
        self.conversation_event("central-agent:app-server-skills",json!({"owner":owner,"view":self.app_server.skills.views.get(owner),"selected":self.app_server.skills.selected.get(owner).cloned().unwrap_or_default(),"extraRoots":self.app_server.skills.extra_roots,"computerUse":self.app_server.view.computer_use,"browserUse":self.app_server.view.browser_use,"computerUseConnection":self.app_server.view.computer_use_connection,"computerUseError":self.app_server.view.errors.get("computer-use"),"computerUseCanReconnect":self.can_reconnect_computer_use(),"writing":self.app_server.native_config_pending(),"notice":self.app_server.skills.notice,"error":error}));
    }
}

pub(super) fn append_skill_inputs(
    input: &mut Vec<Value>,
    skills: &[SelectedSkill],
    directory: &Path,
) -> Result<(), String> {
    let mut paths = HashSet::new();
    for skill in skills {
        if !same_directory(&skill.directory, directory)
            || !Path::new(&skill.path).is_absolute()
            || skill.name.is_empty()
            || skill.name.chars().any(char::is_whitespace)
            || skill.name.contains('\0')
        {
            return Err("The selected skill reference no longer matches the native project".into());
        }
        if paths.insert(&skill.path) {
            input.push(api::text_input(&format!("${}", skill.name)));
            input.push(api::skill_input(&skill.name, &skill.path));
        }
    }
    Ok(())
}

impl State {
    pub(in crate::browser) fn capture_skills_for_prompt(
        &self,
        owner: &str,
        directory: Option<&Path>,
        prompt: &str,
    ) -> Result<Vec<SelectedSkill>, String> {
        let mut selected = self.skills.capture(owner, directory)?;
        if self.view.computer_use != "available" {
            return Ok(selected);
        }
        let Some((root, directory)) = self.skills.official_root.as_ref().zip(directory) else {
            return Ok(selected);
        };
        let path = root.join("computer-use").join("SKILL.md");
        if explicit_computer_use_request(prompt)
            && self.skills.automatic_allowed(owner, &path)
            && path.is_file()
            && !selected
                .iter()
                .any(|skill| same_directory(Path::new(&skill.path), &path))
        {
            selected.push(SelectedSkill {
                id: Uuid::new_v4().to_string(),
                name: "computer-use:computer-use".into(),
                path: display_path(&path),
                directory: directory.to_path_buf(),
                current: true,
            });
        }
        Ok(selected)
    }
    pub(in crate::browser) fn promote_skills(&mut self, from: &str, to: &str) {
        self.skills.promote(from, to);
    }
    pub(in crate::browser) fn consume_skills(&mut self, owner: &str, accepted: &[SelectedSkill]) {
        self.skills.consume(owner, accepted);
    }
    pub(in crate::browser) fn native_config_pending(&self) -> bool {
        self.skills.writing() || self.preferences.writing() || self.mcp.writing()
    }
}

// Only a direct request to operate the desktop opts into this official skill.
// Mentions in explanations and negated requests leave invocation to the user.
fn explicit_computer_use_request(prompt: &str) -> bool {
    static REFERENCE: OnceLock<Regex> = OnceLock::new();
    static INVOKE: OnceLock<Regex> = OnceLock::new();
    static EXPLAIN: OnceLock<Regex> = OnceLock::new();
    static ACTION: OnceLock<Regex> = OnceLock::new();
    static WITH_TOOL: OnceLock<Regex> = OnceLock::new();
    let reference = REFERENCE.get_or_init(|| Regex::new(r"\bcomputer[ -]use\b").unwrap());
    let invoke = INVOKE.get_or_init(|| {
        Regex::new(r"\b(?:usa|usare|usiamo|utilizza|utilizzare|impiega|avvia|attiva|use|using|invoke|enable)\b").unwrap()
    });
    let explain = EXPLAIN.get_or_init(|| {
        Regex::new(r"\b(?:spiegami|spiega|explain|how|what|why|come|cosa|quali)\b").unwrap()
    });
    let action = ACTION.get_or_init(|| {
        Regex::new(r"\b(?:apri|clicca|premi|controlla|leggi|mostra|procedi|esegui|open|click|press|inspect|read|show|launch)\b").unwrap()
    });
    let with_tool = WITH_TOOL.get_or_init(|| {
        Regex::new(r"\b(?:con|tramite|mediante|via|through|using|with)\b").unwrap()
    });
    let lower = prompt.to_lowercase();
    reference.find_iter(&lower).any(|mention| {
        let clause_start = lower[..mention.start()]
            .rfind(['.', '!', '?', ';', '\n'])
            .map_or(0, |index| index + 1);
        let clause_end = lower[mention.end()..]
            .find(['.', '!', '?', ';', '\n'])
            .map_or(lower.len(), |index| mention.end() + index);
        let before = &lower[clause_start..mention.start()];
        let after = &lower[mention.end()..clause_end];
        let direct = invoke.find_iter(before).last().is_some_and(|verb| {
            let context = &before[..verb.start()];
            let since_comma = context.rsplit(',').next().unwrap_or(context);
            !explain.is_match(context)
                && !["non", "senza", "don't", "do not", "never", "without"]
                    .iter()
                    .any(|negation| since_comma.contains(negation))
                && before[verb.end()..].chars().count() <= 100
        });
        let nearby = before.chars().rev().take(55).collect::<String>();
        let nearby = nearby.chars().rev().collect::<String>();
        let via_action = action.is_match(before)
            && with_tool.is_match(&nearby)
            && !explain.is_match(before)
            && !before.contains("non ")
            && !before.contains("do not ");
        let directed = after.trim_start().starts_with(':')
            && action.is_match(&after.chars().take(60).collect::<String>())
            && !explain.is_match(before)
            && !after.contains("non ")
            && !after.contains("do not ")
            && !after.contains("don't ");
        direct || via_action || directed
    })
}

fn same_directory(a: &Path, b: &Path) -> bool {
    a == b
        || fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn computer_use_fixture(root: &Path) -> State {
        let official = root.join("official");
        let path = official.join("computer-use/SKILL.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "official fixture").unwrap();
        let mut state = State::default();
        state.view.computer_use = "available";
        state
            .skills
            .set_computer_use(Some(&computer_use::Attachment {
                root: Some(official),
                enabled: true,
                browser_root: None,
                browser_enabled: false,
            }));
        state
    }

    #[test]
    fn computer_use_uses_only_official_skill_in_both_composers_and_queue() {
        let temp = tempfile::tempdir().unwrap();
        for owner in ["chat:main", "graph:node"] {
            let mut state = computer_use_fixture(temp.path());
            for prompt in [
                "Explain Computer Use",
                "Non usare Computer Use",
                "Read a file",
            ] {
                assert!(
                    state
                        .capture_skills_for_prompt(owner, Some(temp.path()), prompt)
                        .unwrap()
                        .is_empty()
                );
            }
            let auto = state
                .capture_skills_for_prompt(
                    owner,
                    Some(temp.path()),
                    "Usa Computer Use per aprire Calcolatrice.",
                )
                .unwrap();
            assert_eq!(auto.len(), 1);
            assert_eq!(auto[0].name, "computer-use:computer-use");
            state.skills.selected.insert(owner.into(), auto.clone());
            let manual = state
                .capture_skills_for_prompt(owner, Some(temp.path()), "Calculate 4 + 1")
                .unwrap();
            assert_eq!(manual.len(), 1);
            assert_eq!(manual[0].id, auto[0].id);
            let restored: Vec<SelectedSkill> =
                serde_json::from_str(&serde_json::to_string(&manual).unwrap()).unwrap();
            let mut wire = vec![api::text_input("Calculate 4 + 1")];
            append_skill_inputs(&mut wire, &restored, temp.path()).unwrap();
            assert_eq!(wire.len(), 3);
            assert_eq!(
                wire[2],
                api::skill_input(&restored[0].name, &restored[0].path)
            );
            assert!(
                append_skill_inputs(&mut vec![], &restored, &temp.path().join("other-project"))
                    .is_err()
            );
            state.skills.selected.clear();
            state.view.computer_use = "unavailable";
            assert!(
                state
                    .capture_skills_for_prompt(owner, Some(temp.path()), "Usa Computer Use")
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn official_browser_selection_is_owner_scoped_and_uses_the_native_skill_input() {
        let temp = tempfile::tempdir().unwrap();
        let computer = temp.path().join("computer-skills");
        let browser = temp.path().join("browser-skills");
        fs::create_dir_all(computer.join("computer-use")).unwrap();
        fs::create_dir_all(browser.join("control-in-app-browser")).unwrap();
        fs::write(computer.join("computer-use/SKILL.md"), "computer").unwrap();
        fs::write(browser.join("control-in-app-browser/SKILL.md"), "browser").unwrap();
        let mut skills = Skills::default();
        skills.set_computer_use(Some(&computer_use::Attachment {
            root: Some(computer),
            enabled: true,
            browser_root: Some(browser),
            browser_enabled: true,
        }));
        skills
            .attach_official_browser("chat:main", temp.path())
            .unwrap();
        skills
            .attach_official_browser("chat:main", temp.path())
            .unwrap();
        let selected = skills.capture("chat:main", Some(temp.path())).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "browser:control-in-app-browser");
        assert!(
            skills
                .capture("graph:other", Some(temp.path()))
                .unwrap()
                .is_empty()
        );
        let mut input = vec![api::text_input("Inspect localhost")];
        append_skill_inputs(&mut input, &selected, temp.path()).unwrap();
        assert_eq!(input[1]["text"], "$browser:control-in-app-browser");
        assert_eq!(input[2]["type"], "skill");
    }

    #[test]
    fn computer_use_automatic_skill_respects_native_effective_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = computer_use_fixture(temp.path());
        let owner = "chat:test";
        let path = state
            .skills
            .official_root
            .as_ref()
            .unwrap()
            .join("computer-use/SKILL.md");
        for (effective, expected) in [(Some(false), 0), (None, 0), (Some(true), 1)] {
            state.skills.write = Some(Write {
                id: "write".into(),
                owner: owner.into(),
                path: display_path(&path),
                enabled: true,
            });
            state
                .skills
                .reply(owner, "write", Ok(json!({"effectiveEnabled": effective})));
            assert_eq!(
                state
                    .capture_skills_for_prompt(owner, Some(temp.path()), "Usa Computer Use")
                    .unwrap()
                    .len(),
                expected
            );
        }
        state.skills.set_computer_use(None);
        assert!(state.skills.official_root.is_none());
    }

    #[test]
    fn computer_use_is_attached_only_to_an_explicit_desktop_request() {
        let temp = tempfile::tempdir().unwrap();
        let official = temp.path().join("official");
        let skill = official.join("computer-use").join("SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "official fixture").unwrap();
        let mut state = State::default();
        state.view.computer_use = "available";
        state.skills.official_root = Some(official);
        let owner = "chat:test";
        let directory = Some(temp.path());
        let explicit = state
            .capture_skills_for_prompt(
                owner,
                directory,
                "Usa il plugin ufficiale Computer Use per aprire Calcolatrice.",
            )
            .unwrap();
        assert_eq!(explicit.len(), 1);
        assert_eq!(explicit[0].name, "computer-use:computer-use");
        let mut input = vec![api::text_input("Open Calculator")];
        append_skill_inputs(&mut input, &explicit, temp.path()).unwrap();
        assert_eq!(input[1]["text"], "$computer-use:computer-use");
        assert_eq!(
            input[2],
            api::skill_input(&explicit[0].name, &explicit[0].path)
        );

        for actionable in [
            "Apri Calcolatrice con Computer Use.",
            "Non usare il terminale, usa Computer Use per aprire Calcolatrice.",
            "Computer Use: apri Calcolatrice.",
            "Can you use Computer Use to open Calculator?",
        ] {
            assert_eq!(
                state
                    .capture_skills_for_prompt(owner, directory, actionable)
                    .unwrap()
                    .len(),
                1,
                "{actionable}"
            );
        }

        for explanatory in [
            "Spiegami come funziona Computer Use.",
            "Non usare Computer Use per questa richiesta.",
            "What is the Computer Use plugin?",
            "Cosa possiamo fare con Computer Use?",
            "Come si usa Computer Use?",
            "Computer Use: non aprire Calcolatrice.",
        ] {
            assert!(
                state
                    .capture_skills_for_prompt(owner, directory, explanatory)
                    .unwrap()
                    .is_empty()
            );
        }
        state.view.computer_use = "unavailable";
        assert!(
            state
                .capture_skills_for_prompt(owner, directory, "Usa Computer Use.")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn automatic_computer_use_does_not_duplicate_a_manual_selection() {
        let temp = tempfile::tempdir().unwrap();
        let official = temp.path().join("official");
        let skill = official.join("computer-use").join("SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "official fixture").unwrap();
        let mut state = State::default();
        state.view.computer_use = "available";
        state.skills.official_root = Some(official);
        state.skills.selected.insert(
            "chat:test".into(),
            vec![SelectedSkill {
                id: "manual".into(),
                name: "computer-use:computer-use".into(),
                path: display_path(&skill),
                directory: temp.path().into(),
                current: true,
            }],
        );
        let selected = state
            .capture_skills_for_prompt(
                "chat:test",
                Some(temp.path()),
                "Usa Computer Use per cliccare il pulsante.",
            )
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "manual");
    }
    fn fixture(root: &Path) -> Value {
        let mut samples: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-skills.json"
        )))
        .unwrap();
        let data = &mut samples[0]["params"]["data"][0];
        data["cwd"] = json!(root);
        data["skills"][0]["path"] = json!(root.join(".agents/skills/fixture/SKILL.md"));
        data["skills"][0]["interface"] = json!({"defaultPrompt":"never-publish-instructions","iconLarge":"https://invalid.test/private-icon"});
        samples[0]["params"].clone()
    }

    #[test]
    fn explicit_skill_selection_is_inventory_owned_enabled_and_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        let path = state.views["chat:a"].items[0].path.clone();
        assert!(state.attach("graph:b", &view, &path, root).is_err());
        assert!(state.attach("chat:a", "old", &path, root).is_err());
        assert!(
            state
                .attach("chat:a", &view, "C:/not-selected/SKILL.md", root)
                .is_err()
        );
        assert!(
            state
                .attach("chat:a", &view, &path, &root.join("other"))
                .is_err()
        );
        state.views.get_mut("chat:a").unwrap().items[0].enabled = false;
        assert!(state.attach("chat:a", &view, &path, root).is_err());
        state.views.get_mut("chat:a").unwrap().items[0].enabled = true;
        state.attach("chat:a", &view, &path, root).unwrap();
        state.attach("chat:a", &view, &path, root).unwrap();
        assert_eq!(state.capture("chat:a", Some(root)).unwrap().len(), 1);
        assert!(state.capture("graph:b", Some(root)).unwrap().is_empty());
        assert!(state.capture("chat:a", Some(&root.join("other"))).is_err());
        assert!(!state.writing());
    }

    #[test]
    fn skill_views_serialize_the_same_display_directory_as_the_conversation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(temp.path()).unwrap();
        let mut state = ready(&canonical);
        let view = state.views["chat:a"].view_id.clone();
        let path = state.views["chat:a"].items[0].path.clone();
        state.attach("chat:a", &view, &path, &canonical).unwrap();

        let inventory = serde_json::to_value(&state.views["chat:a"]).unwrap();
        let selected = serde_json::to_value(&state.selected["chat:a"][0]).unwrap();
        let displayed = display_path(&canonical);
        assert_eq!(inventory["directory"], displayed);
        assert_eq!(selected["directory"], displayed);
    }

    #[test]
    fn selected_skill_invalidation_requires_native_refresh_or_explicit_removal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        let path = state.views["chat:a"].items[0].path.clone();
        state.attach("chat:a", &view, &path, root).unwrap();
        let selected_id = state.selected["chat:a"][0].id.clone();
        state.close("chat:a", "ui-a");
        assert_eq!(state.capture("chat:a", Some(root)).unwrap().len(), 1);
        state.invalidate();
        assert!(state.capture("chat:a", Some(root)).is_err());
        let (read, _) = state
            .refresh("chat:a", "refresh".into(), root.into())
            .unwrap();
        state.reply("chat:a", &read, Ok(fixture(root)));
        assert_eq!(
            state.capture("chat:a", Some(root)).unwrap()[0].id,
            selected_id
        );
        state.disconnect();
        assert!(state.capture("chat:a", Some(root)).is_err());
        state.remove("graph:b", &selected_id);
        assert_eq!(state.selected["chat:a"].len(), 1);
        state.remove("chat:a", &selected_id);
        assert!(state.capture("chat:a", Some(root)).unwrap().is_empty());
    }

    #[test]
    fn skill_snapshot_is_independent_of_later_selections_and_consumes_exact_ids() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        let path = state.views["chat:a"].items[0].path.clone();
        state.attach("chat:a", &view, &path, root).unwrap();
        let frozen = state.capture("chat:a", Some(root)).unwrap();
        state.remove("chat:a", &frozen[0].id);
        state.attach("chat:a", &view, &path, root).unwrap();
        let next_id = state.selected["chat:a"][0].id.clone();
        assert_ne!(next_id, frozen[0].id);
        state.consume("chat:a", &frozen);
        assert_eq!(state.selected["chat:a"][0].id, next_id);
        state.promote("chat:a", "chat:promoted");
        assert!(state.capture("chat:a", Some(root)).unwrap().is_empty());
        let promoted = state.capture("chat:promoted", Some(root)).unwrap();
        assert_eq!(promoted[0].id, next_id);
        state.consume("chat:promoted", &promoted);
        assert!(
            state
                .capture("chat:promoted", Some(root))
                .unwrap()
                .is_empty()
        );
        let mut input = vec![api::text_input("Inspect this project")];
        append_skill_inputs(&mut input, &frozen, &fs::canonicalize(root).unwrap()).unwrap();
        assert_eq!(input.len(), 3);
        assert_eq!(input[1]["text"], format!("${}", frozen[0].name));
        assert_eq!(input[2], api::skill_input(&frozen[0].name, &frozen[0].path));
        assert!(
            !serde_json::to_string(&input)
                .unwrap()
                .contains("never-publish-instructions")
        );
        assert!(append_skill_inputs(&mut vec![], &frozen, &root.join("other")).is_err());
    }
    fn ready(root: &Path) -> Skills {
        let mut state = Skills::default();
        let (id, call) = state.refresh("chat:a", "ui-a".into(), root.into()).unwrap();
        assert_eq!(call.method, "skills/list");
        assert_eq!(call.params, json!({"cwds":[root],"forceReload":true}));
        state.reply("chat:a", &id, Ok(fixture(root)));
        assert!(state.views["chat:a"].current);
        state
    }
    #[test]
    fn native_skill_projection_contains_metadata_and_discovery_errors_only() {
        let temp = tempfile::tempdir().unwrap();
        let state = ready(temp.path());
        let view = &state.views["chat:a"];
        assert_eq!(view.items[0].name, "fixture-skill");
        assert!(view.items[0].enabled);
        assert_eq!(view.errors.len(), 1);
        let serialized = serde_json::to_string(view).unwrap();
        assert!(!serialized.contains("never-publish-instructions"));
        assert!(!serialized.contains("private-icon"));
        assert!(!serialized.contains("pending"));
    }
    #[test]
    fn invalid_skill_identities_and_different_directories_never_publish_choices() {
        let temp = tempfile::tempdir().unwrap();
        for invalid in ["cwd", "relative", "duplicate", "scope", "missing"] {
            let mut state = Skills::default();
            let (id, _) = state
                .refresh("chat:a", "ui".into(), temp.path().into())
                .unwrap();
            let mut value = fixture(temp.path());
            match invalid {
                "cwd" => value["data"][0]["cwd"] = json!(temp.path().join("other")),
                "relative" => value["data"][0]["skills"][0]["path"] = json!("relative/SKILL.md"),
                "scope" => value["data"][0]["skills"][0]["scope"] = json!("unknown"),
                "duplicate" => {
                    let item = value["data"][0]["skills"][0].clone();
                    value["data"][0]["skills"]
                        .as_array_mut()
                        .unwrap()
                        .push(item);
                }
                _ => {
                    value["data"][0].as_object_mut().unwrap().remove("skills");
                }
            }
            state.reply("chat:a", &id, Ok(value));
            assert!(!state.views["chat:a"].current, "{invalid}");
            assert!(state.views["chat:a"].error.is_some(), "{invalid}");
            assert!(state.views["chat:a"].items.is_empty());
        }
    }
    #[test]
    fn stale_reads_closed_views_and_native_invalidation_cannot_restore_authority() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = ready(temp.path());
        let (old, _) = state
            .refresh("chat:a", "old-ui".into(), temp.path().into())
            .unwrap();
        let (new, _) = state
            .refresh("chat:a", "new-ui".into(), temp.path().into())
            .unwrap();
        state.close("chat:a", "old-ui");
        state.reply("graph:b", &new, Ok(fixture(temp.path())));
        state.reply("chat:a", &old, Ok(fixture(temp.path())));
        assert!(state.views["chat:a"].loading);
        state.invalidate();
        state.reply("chat:a", &new, Ok(fixture(temp.path())));
        assert!(!state.views["chat:a"].current);
        state.close("chat:a", "new-ui");
        state.reply("chat:a", &new, Ok(fixture(temp.path())));
        assert!(state.views.is_empty());
    }
    #[test]
    fn native_write_requires_exact_inventory_owner_path_directory_and_changed_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = ready(temp.path());
        let view_id = state.views["chat:a"].view_id.clone();
        let path = state.views["chat:a"].items[0].path.clone();
        assert!(
            state
                .choose("graph:other", &view_id, &path, false, temp.path())
                .is_err()
        );
        assert!(
            state
                .choose("chat:a", "stale", &path, false, temp.path())
                .is_err()
        );
        assert!(
            state
                .choose("chat:a", &view_id, "foreign", false, temp.path())
                .is_err()
        );
        assert!(
            state
                .choose("chat:a", &view_id, &path, true, temp.path())
                .is_err()
        );
        assert!(
            state
                .choose("chat:a", &view_id, &path, false, &temp.path().join("other"))
                .is_err()
        );
        let (id, call) = state
            .choose("chat:a", &view_id, &path, false, temp.path())
            .unwrap();
        assert_eq!(call.method, "skills/config/write");
        assert_eq!(call.params, json!({"path":path,"enabled":false}));
        assert!(state.writing());
        assert!(!state.views["chat:a"].current);
        assert!(
            state
                .choose("chat:a", &view_id, &path, false, temp.path())
                .is_err()
        );
        state.close("chat:a", "ui-a");
        assert!(state.writing());
        state.reply("graph:foreign", &id, Ok(json!({"effectiveEnabled":false})));
        assert!(state.writing());
        state.reply("chat:a", &id, Ok(json!({"effectiveEnabled":true})));
        assert!(!state.writing());
        assert!(state.notice.as_ref().unwrap().contains("differs"));
    }
    #[test]
    fn write_errors_or_disconnect_require_refresh_without_replay() {
        let temp = tempfile::tempdir().unwrap();
        for outcome in ["disconnect", "unknown", "rejected", "invalid"] {
            let mut host = State {
                skills: ready(temp.path()),
                ..State::default()
            };
            let view_id = host.skills.views["chat:a"].view_id.clone();
            let path = host.skills.views["chat:a"].items[0].path.clone();
            let (id, _) = host
                .skills
                .choose("chat:a", &view_id, &path, false, temp.path())
                .unwrap();
            assert!(host.any_busy());
            match outcome {
                "disconnect" => {
                    host.skills.disconnect();
                    host.skills
                        .reply("chat:a", &id, Ok(json!({"effectiveEnabled":false})));
                }
                "unknown" => host.skills.reply(
                    "chat:a",
                    &id,
                    Err(CallError::Disconnected {
                        reason: "closed".into(),
                        delivery_unknown: true,
                    }),
                ),
                "rejected" => host.skills.reply(
                    "chat:a",
                    &id,
                    Err(CallError::Rejected("Policy denies this setting".into())),
                ),
                _ => host.skills.reply("chat:a", &id, Ok(json!({}))),
            }
            assert!(!host.any_busy());
            assert!(!host.skills.views["chat:a"].current);
            assert!(host.skills.notice.is_some());
            assert!(
                host.skills
                    .choose("chat:a", &view_id, &path, false, temp.path())
                    .is_err()
            );
        }
    }

    #[test]
    fn extra_roots_are_canonical_process_scoped_and_invalidate_discovery() {
        let project = tempfile::tempdir().unwrap();
        let extra = tempfile::tempdir().unwrap();
        let mut skills = ready(project.path());
        let wire = extra.path().display().to_string();
        let (id, call) = skills
            .set_extra_roots("chat:a", vec![wire.clone(), wire])
            .unwrap();
        assert_eq!(call.method, "skills/extraRoots/set");
        assert_eq!(call.params["extraRoots"].as_array().unwrap().len(), 1);
        assert!(skills.writing());
        assert!(!skills.views["chat:a"].current);
        skills.reply("graph:foreign", &id, Ok(json!({})));
        assert!(skills.writing());
        skills.reply("chat:a", &id, Ok(json!({})));
        assert!(!skills.writing());
        assert_eq!(
            skills.extra_roots,
            vec![fs::canonicalize(extra.path()).unwrap()]
        );
        assert!(skills.notice.as_deref().unwrap().contains("process-scoped"));

        assert!(
            skills
                .set_extra_roots("chat:a", vec!["relative".into()])
                .is_err()
        );
        let sensitive = project.path().join(".ssh");
        fs::create_dir(&sensitive).unwrap();
        assert!(
            skills
                .set_extra_roots("chat:a", vec![sensitive.display().to_string()])
                .unwrap_err()
                .contains("secret directories")
        );
        let (id, _) = skills
            .set_extra_roots("chat:a", vec![extra.path().display().to_string()])
            .unwrap();
        skills.disconnect();
        skills.reply("chat:a", &id, Ok(json!({})));
        assert!(skills.extra_roots.is_empty());
        assert!(skills.notice.as_deref().unwrap().contains("unknown"));
    }

    #[test]
    fn user_extra_roots_keep_the_official_plugin_skill_root() {
        let project = tempfile::tempdir().unwrap();
        let official = tempfile::tempdir().unwrap();
        let extra = tempfile::tempdir().unwrap();
        let mut skills = ready(project.path());
        skills.official_root = Some(fs::canonicalize(official.path()).unwrap());
        skills.official_bootstrap_pending = true;
        assert!(
            skills
                .set_extra_roots("chat:a", vec![extra.path().display().to_string()])
                .is_err()
        );
        skills.official_bootstrap_pending = false;
        let (id, call) = skills
            .set_extra_roots("chat:a", vec![extra.path().display().to_string()])
            .unwrap();
        let roots = call.params["extraRoots"].as_array().unwrap();
        assert_eq!(roots.len(), 2);
        assert!(
            roots.iter().any(|root| root.as_str()
                == Some(skills.official_root.as_ref().unwrap().to_str().unwrap()))
        );
        skills.reply("chat:a", &id, Ok(json!({})));
        assert_eq!(
            skills.extra_roots,
            vec![fs::canonicalize(extra.path()).unwrap()]
        );
    }
}
