//! Opt-in real native approval. Never authorizes an arbitrary model command.
use super::*;
use central_agent_codex_runtime::requests::Kind;
use central_agent_codex_runtime::{requests::Decision, wire::ServerRequestKey};

const MARKER: &str = "NATIVE_APPROVAL_ALLOWED";
const DRAFT: &str = "Unsent draft survives native approval";
const SIBLING_DRAFT: &str = "Unsent sibling is independent of approval";

pub(super) fn root_script(owner: &str) -> String {
    if let Some(key) = owner.strip_prefix("graph:") {
        format!(
            "const root=[...document.querySelectorAll('.agent-console[data-node-key]')].find(card=>card.dataset.nodeKey==={});const input=root?.querySelector('textarea[data-role=\"message\"]');",
            json!(key)
        )
    } else {
        "const root=document;const input=document.getElementById('chat-input');".into()
    }
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(() => {{ {} return {{renderer:typeof window.{}, ready:!!input,
      draft:input?.value,
      cards:[...root?.querySelectorAll('.native-requests article') || []].map(card=>({{title:card.getAttribute('aria-label'),text:card.textContent,busy:card.getAttribute('aria-busy'),diff:[...card.querySelectorAll('.diff-code')].map(code=>code.textContent).join('\n')}})),
      diffs:[...root?.querySelectorAll('.chat-message .diff-code')||[]].map(code=>code.textContent).join('\n'),
      text:[...root?.querySelectorAll('.chat-message.assistant') || []].map(row=>row.textContent).join('\n'),
      siblings:[...document.querySelectorAll('.agent-console[data-node-key]')].filter(card=>card!==root).map(card=>({{draft:card.querySelector('textarea[data-role="message"]')?.value,requests:card.querySelectorAll('.native-requests article').length}})),
      errors:window.__nativeAcceptanceErrors || []}}; }})()"#,
        root_script(owner),
        if owner.starts_with("graph:") {
            "renderAgentGraphSurfaceState"
        } else {
            "renderAgentPanelState"
        }
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Choice {
    #[default]
    Allow,
    Session,
    Decline,
    Cancel,
}
impl Choice {
    pub(super) fn parse(
        approval: bool,
        decline: bool,
        cancel: bool,
        session: bool,
    ) -> anyhow::Result<Self> {
        let choices = [decline, cancel, session]
            .into_iter()
            .filter(|selected| *selected)
            .count();
        anyhow::ensure!(
            choices <= 1 && (approval || choices == 0),
            "Use at most one of --decline, --cancel or --session, and only with --approvals"
        );
        Ok(if decline {
            Self::Decline
        } else if cancel {
            Self::Cancel
        } else if session {
            Self::Session
        } else {
            Self::Allow
        })
    }
    fn button(self) -> &'static str {
        match self {
            Self::Allow => "Allow once",
            Self::Session => "Allow for session",
            Self::Decline => "Decline",
            Self::Cancel => "Cancel request",
        }
    }
    fn allows(self) -> bool {
        matches!(self, Self::Allow | Self::Session)
    }
    fn decision_matches(self, decision: &Decision) -> bool {
        matches!(
            (self, decision),
            (Self::Allow, Decision::Accept {})
                | (Self::Session, Decision::Session {})
                | (Self::Decline, Decision::Decline {})
                | (Self::Cancel, Decision::Cancel {})
        )
    }
    fn terminal(self, status: &str) -> bool {
        status == "completed" || self == Self::Cancel && status == "interrupted"
    }
    fn matches(self, item: &Value) -> bool {
        if item["type"] != "commandExecution" {
            return false;
        }
        if self.allows() {
            item["status"] == "completed"
                && item["exitCode"] == 0
                && item["aggregatedOutput"]
                    .as_str()
                    .is_some_and(|output| output.trim() == MARKER)
        } else {
            item["status"] == "declined" && item["exitCode"] != 0
        }
    }
}

#[derive(Default)]
pub(super) struct Approvals {
    choice: Choice,
    graph_opened: bool,
    sibling_ready: bool,
    directory: Option<PathBuf>,
    phase: u8,
    thread: Option<String>,
    item: Option<String>,
    ticket: Option<String>,
    request_key: Option<ServerRequestKey>,
    selected: bool,
    requests: usize,
    answered: bool,
    resolved: bool,
    read: bool,
    error: Option<String>,
    file_change: bool,
    diff_seen: bool,
}

fn body() -> String {
    // String is a core type permitted by Windows ConstrainedLanguage. Console
    // method invocation is not; never relax the sandbox just to pass this probe.
    format!("[string]::Concat('{MARKER}')")
}
// Exact command grammar, not substring matching. No shell suffix, profile,
// environment expansion, extra flags or alternative executable is approved.
fn print_only(command: &str, executables: &[PathBuf]) -> bool {
    if command == body() {
        return true;
    }
    executables.iter().any(|path| {
        let executable = path.display().to_string().replace('/', "\\");
        // Codex's display formatter can JSON-quote Windows paths. Accept only
        // the exact known path in those representations, never arbitrary JSON.
        [
            executable.clone(),
            format!("\"{executable}\""),
            json!(executable).to_string(),
        ]
        .iter()
        .any(|exe| {
            [
                format!("{exe} -NoProfile -Command \"{}\"", body()),
                format!("{exe} -NoProfile -NonInteractive -Command \"{}\"", body()),
            ]
            .iter()
            .any(|expected| command.eq_ignore_ascii_case(expected))
        })
    })
}
pub(super) fn script(app: &BrowserApp, owner: &str, body: &str) -> anyhow::Result<()> {
    let panel = if owner.starts_with("graph:") {
        &app.agent_graph_surface
    } else {
        &app.agent_panel
    };
    panel.as_ref().context("Missing approval test panel")?
        .evaluate_script(&format!("try {{ {} {body} }} catch(error) {{ (window.__nativeAcceptanceErrors ||= []).push(String(error)); }}", root_script(owner)))?;
    Ok(())
}
impl Approvals {
    pub(super) fn new(choice: Choice, file_change: bool) -> Self {
        Self {
            choice,
            file_change,
            ..Self::default()
        }
    }
    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        match event {
            BrowserEvent::AppServer(Event::Transport {
                event:
                    TransportEvent::ServerRequest {
                        key,
                        method,
                        params,
                    },
                ..
            }) if app
                .app_server
                .conversations
                .binding(owner)
                .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
            {
                self.requests += 1;
                let expected = if self.file_change {
                    "item/fileChange/requestApproval"
                } else {
                    "item/commandExecution/requestApproval"
                };
                if method != expected || self.requests != 1 {
                    self.error =
                        Some("Unexpected/repeated native request; no approval will be sent".into());
                }
                self.item = params["itemId"].as_str().map(str::to_owned);
                self.request_key = Some(key.clone());
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "turn/diff/updated"
                && app
                    .app_server
                    .conversations
                    .binding(owner)
                    .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
            {
                self.diff_seen = self.diff_seen || file_changes::rendered(&params["diff"]);
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "serverRequest/resolved"
                && app
                    .app_server
                    .conversations
                    .binding(owner)
                    .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
            {
                if self.request_key.as_ref().is_some_and(|key| {
                    serde_json::to_value(&key.id).ok().as_ref() == Some(&params["requestId"])
                }) {
                    self.resolved = true;
                } else {
                    self.error = Some("Native approval resolution targeted another request".into());
                }
            }
            BrowserEvent::AgentPanel(AgentPanelMessage::AppServerRequest {
                owner: reply_owner,
                action: requests::RequestAction::Answer { ticket, decision },
            })
            | BrowserEvent::ScopedAgentPanel {
                message:
                    AgentPanelMessage::AppServerRequest {
                        owner: reply_owner,
                        action: requests::RequestAction::Answer { ticket, decision },
                    },
                ..
            } if reply_owner == owner => {
                if self.ticket.as_ref() != Some(ticket) || !self.choice.decision_matches(decision) {
                    self.error =
                        Some("Actual UI selected another native decision or ticket".into());
                } else {
                    self.selected = true;
                }
            }
            BrowserEvent::AppServer(Event::Answered {
                owner: reply_owner,
                key,
                result,
                ..
            }) if reply_owner == owner => {
                self.answered = result.is_ok() && self.request_key.as_ref() == Some(key);
                if self.request_key.as_ref() != Some(key) {
                    self.error =
                        Some("Native answer acknowledgement targeted another request".into());
                }
                if let Err(error) = result {
                    self.error = Some(error.to_string());
                }
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner => {
                if let Err(error) = result {
                    self.error = Some(format!("Native {} failed: {error}", request.call.method));
                } else if request.call.method == "thread/read" {
                    let value = result.as_ref().unwrap();
                    if value["thread"]["cwd"]
                        .as_str()
                        .and_then(|cwd| fs::canonicalize(cwd).ok())
                        != self
                            .directory
                            .as_ref()
                            .and_then(|path| fs::canonicalize(path).ok())
                    {
                        self.error = Some("Native approval readback used another directory".into());
                    }
                    self.read = true;
                }
            }
            _ => {}
        }
    }
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        sample: &Value,
        graph: Option<&graph::Graph>,
    ) -> anyhow::Result<bool> {
        if let Some(error) = &self.error {
            anyhow::bail!("{error}");
        }
        if sample.is_null() {
            return Ok(false);
        }
        anyhow::ensure!(
            sample["renderer"] == "function",
            "Approval renderer missing"
        );
        if graph.is_some() && self.phase > 0 {
            let siblings = sample["siblings"]
                .as_array()
                .context("Missing sibling sample")?;
            if !self.sibling_ready && (siblings.len() != 1 || siblings[0]["draft"] != SIBLING_DRAFT)
            {
                return Ok(false);
            }
            self.sibling_ready = true;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["draft"] == SIBLING_DRAFT
                    && siblings[0]["requests"] == 0,
                "Approval changed or entered another graph card"
            );
        }
        let directory = graph
            .and_then(|graph| graph.directory(owner))
            .or_else(|| app.workspace.root())
            .context("Missing approval fixture directory")?
            .to_path_buf();
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            self.directory = Some(directory.clone());
            if let Some(graph) = graph {
                if !self.graph_opened {
                    graph.open_cards(app)?;
                    self.graph_opened = true;
                    return Ok(false);
                }
                if sample["ready"] != true
                    || sample["siblings"]
                        .as_array()
                        .is_none_or(|siblings| siblings.len() != 1)
                {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    &format!(
                        "for(const card of document.querySelectorAll('.agent-console[data-node-key]')) {{ if(card!==root) {{ const other=card.querySelector('textarea[data-role=\"message\"]');other.value={};other.dispatchEvent(new Event('input',{{bubbles:true}})); }} }}",
                        json!(SIBLING_DRAFT)
                    ),
                )?;
            } else {
                app.render_agent_panel();
            }
            let prompt = if self.file_change {
                file_changes::prepare(&directory)?
            } else {
                format!(
                    "Use the shell tool exactly once to execute this exact PowerShell command: {}. Use the current working directory and login=false; do not add commands, read files, change files, request extra permissions or change configuration. Wait for any native command approval. After execution report its stdout. If declined, do not retry.",
                    body()
                )
            };
            script(
                app,
                owner,
                &format!(
                    "input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));{}",
                    json!(prompt),
                    if graph.is_some() {
                        "root.querySelector('form.agent-console-compose').requestSubmit();"
                    } else {
                        "input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));"
                    }
                ),
            )?;
            self.phase = 1;
            println!(
                "Approval host: submitted one exact {} prompt.",
                if self.file_change {
                    "single-file replacement"
                } else {
                    "print-only command"
                }
            );
            return Ok(false);
        }
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        if let Some(thread) = &self.thread {
            anyhow::ensure!(
                thread == &binding.thread_id,
                "Approval changed native thread"
            );
        } else {
            self.thread = Some(binding.thread_id.clone());
        }
        let Some(thread) = app
            .app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)
        else {
            return Ok(false);
        };
        anyhow::ensure!(
            thread.turns.len() <= 1 && thread.turns.iter().all(|turn| turn.status != "failed"),
            "Native approval turn failed or duplicated"
        );
        let views = app.app_server.requests.views(owner);
        let cards = sample["cards"]
            .as_array()
            .context("Missing approval DOM sample")?;
        if self.phase == 1 {
            if thread.turns.iter().any(|turn| turn.status == "completed") {
                anyhow::bail!("Runtime completed without exercising native approval");
            }
            let Some(request) = views.first() else {
                return Ok(false);
            };
            anyhow::ensure!(
                views.len() == 1
                    && request.kind
                        == if self.file_change {
                            Kind::Files
                        } else {
                            Kind::Command
                        },
                "Unexpected approval scope"
            );
            if self.file_change {
                let item = thread
                    .turns
                    .iter()
                    .flat_map(|turn| &turn.items)
                    .find(|item| Some(item.id.as_str()) == self.item.as_deref())
                    .context("Missing pending file change")?;
                file_changes::validate(&directory, &item.value, &request.params)?;
                file_changes::disk(&directory, false)?;
                if cards.len() != 1 || !file_changes::rendered(&cards[0]["diff"]) {
                    script(
                        app,
                        owner,
                        "root.querySelectorAll('.native-requests details').forEach(details=>details.open=true);",
                    )?;
                    return Ok(false);
                }
            } else {
                let command = request.params["command"]
                    .as_str()
                    .context("No command preview; no approval granted")?;
                let windows = PathBuf::from(
                    std::env::var_os("SystemRoot").context("Missing system directory")?,
                );
                let profile = PathBuf::from(
                    std::env::var_os("USERPROFILE").context("Missing user directory")?,
                );
                let executables = [
                windows.join("System32/WindowsPowerShell/v1.0/powershell.exe"),
                profile.join(".cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe"),
            ];
                anyhow::ensure!(
                    print_only(command, &executables),
                    "Command does not match print-only approval grammar: {command}"
                );
                anyhow::ensure!(
                    request.params["networkApprovalContext"].is_null()
                        && request.params["kind"] != "writeStdin",
                    "Unexpected network/stdin approval"
                );
                let cwd = request.params["cwd"]
                    .as_str()
                    .and_then(|cwd| fs::canonicalize(cwd).ok());
                anyhow::ensure!(
                    cwd == fs::canonicalize(&directory).ok(),
                    "Approval targeted another directory"
                );
            }
            if cards.len() != 1
                || !cards[0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(if self.file_change {
                        file_changes::AFTER
                    } else {
                        MARKER
                    })
                || sample["draft"] != ""
            {
                return Ok(false);
            }
            self.ticket = Some(request.ticket.clone());
            // Unrelated render must not consume the pending request or draft.
            script(
                app,
                owner,
                &format!(
                    "input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));",
                    json!(DRAFT)
                ),
            )?;
            if graph.is_some() {
                app.render_agent_graph_surface();
            } else {
                app.render_agent_panel();
            }
            self.phase = 2;
            return Ok(false);
        }
        if self.phase == 2 {
            if sample["draft"] != DRAFT || cards.len() != 1 {
                return Ok(false);
            }
            anyhow::ensure!(
                views.len() == 1 && self.ticket.as_deref() == Some(&views[0].ticket),
                "Approval ticket changed across render"
            );
            script(
                app,
                owner,
                &format!(
                    "const card=root.querySelector('.native-requests article'); const button=[...card.querySelectorAll('button')].find(button=>button.textContent==={}); if(!button || button.disabled) throw new Error('Native decision unavailable'); const disclosure=button.closest('details'); if(disclosure) disclosure.open=true; button.click();",
                    json!(self.choice.button())
                ),
            )?;
            self.phase = 3;
            println!(
                "Approval host: verified exact action/scope and stable pending card; clicked {}.",
                self.choice.button()
            );
            return Ok(false);
        }
        if self.phase == 3
            && self.selected
            && self.answered
            && self.resolved
            && thread.turns.len() == 1
            && self.choice.terminal(&thread.turns[0].status)
            && !app.app_server.busy(owner)
        {
            let command = thread.turns[0]
                .items
                .iter()
                .find(|item| Some(item.id.as_str()) == self.item.as_deref())
                .context("Approved native item missing")?;
            anyhow::ensure!(
                thread.turns[0].items.iter().all(|item| {
                    matches!(
                        item.value["type"].as_str(),
                        Some("userMessage" | "agentMessage" | "reasoning" | "plan")
                    ) || item.id == command.id
                }),
                "Approval test performed additional tool work"
            );
            anyhow::ensure!(
                if self.file_change {
                    if self.choice == Choice::Cancel {
                        file_changes::cancelled(&command.value, &thread.turns[0].status)
                    } else {
                        file_changes::outcome(&command.value, self.choice.allows())
                    }
                } else {
                    self.choice.matches(&command.value)
                },
                "Native decision did not produce the required execution/rejection: type={} status={} exit={} output={}",
                command.value["type"],
                command.value["status"],
                command.value["exitCode"],
                command.value["aggregatedOutput"]
            );
            if self.file_change {
                file_changes::validate(&directory, &command.value, &json!({}))?;
                file_changes::disk(&directory, self.choice.allows())?;
                anyhow::ensure!(
                    app.app_server_messages(owner).iter().all(|row| {
                        row["activityStatus"] != "running" && row["streaming"] != true
                    }),
                    "Terminal native turn still has live presentation rows"
                );
                if !file_changes::rendered(&sample["diffs"]) {
                    script(
                        app,
                        owner,
                        "root.querySelectorAll('details').forEach(details=>details.open=true);",
                    )?;
                    return Ok(false);
                }
                if self.choice.allows() {
                    anyhow::ensure!(self.diff_seen, "Missing native aggregate turn diff");
                }
            }
            if !views.is_empty()
                || !cards.is_empty()
                || sample["draft"] != DRAFT
                || self.choice.allows()
                    && !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains(if self.file_change {
                            file_changes::AFTER
                        } else {
                            MARKER
                        })
            {
                return Ok(false);
            }
            app.app_server_conversation(owner.into(), ConversationAction::Read {});
            self.phase = 4;
        } else if self.phase == 4 && self.read {
            anyhow::ensure!(
                thread.turns.len() == 1
                    && self.choice.terminal(&thread.turns[0].status)
                    && thread.turns[0]
                        .items
                        .iter()
                        .find(|item| Some(item.id.as_str()) == self.item.as_deref())
                        .is_some_and(|item| if self.file_change {
                            if self.choice == Choice::Cancel {
                                file_changes::cancelled(&item.value, &thread.turns[0].status)
                            } else {
                                file_changes::outcome(&item.value, self.choice.allows())
                            }
                        } else {
                            self.choice.matches(&item.value)
                        }),
                "Native readback did not retain the actual decision outcome"
            );
            if let Some(graph) = graph {
                for sibling in graph
                    .owners()
                    .into_iter()
                    .filter(|sibling| sibling != owner)
                {
                    anyhow::ensure!(
                        app.app_server.conversations.binding(&sibling).is_none()
                            && app.app_server.requests.views(&sibling).is_empty(),
                        "Approval created work in a sibling graph conversation"
                    );
                }
                let main = app.conversation_key(None);
                anyhow::ensure!(
                    app.app_server.conversations.binding(&main).is_none()
                        && app.app_server.requests.views(&main).is_empty(),
                    "Graph approval entered the main conversation"
                );
            }
            anyhow::ensure!(
                self.requests == 1
                    && app.app_server.requests.views(owner).is_empty()
                    && app.app_server.conversations.saved().unresolved.is_empty(),
                "Approval retained pending work"
            );
            anyhow::ensure!(
                app.project_chats
                    .iter()
                    .all(|chat| chat.messages.is_empty()),
                "Approval copied native transcript into another provider"
            );
            if self.file_change {
                file_changes::disk(&directory, self.choice.allows())?;
            } else {
                anyhow::ensure!(
                    fs::read_dir(&directory)?.next().is_none(),
                    "Print-only test changed workspace"
                );
            }
            if sample["draft"] != DRAFT || !cards.is_empty() {
                return Ok(false);
            }
            println!(
                "Approval host: actual {:?} decision, server resolution, expected execution/rejection, card removal, draft and native readback verified.",
                self.choice
            );
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approval_choices_require_explicit_mode_and_never_accept_execution_for_rejection() {
        assert_eq!(
            Choice::parse(true, false, false, false).unwrap(),
            Choice::Allow
        );
        assert_eq!(
            Choice::parse(true, true, false, false).unwrap(),
            Choice::Decline
        );
        assert_eq!(
            Choice::parse(true, false, true, false).unwrap(),
            Choice::Cancel
        );
        assert_eq!(
            Choice::parse(true, false, false, true).unwrap(),
            Choice::Session
        );
        for (approval, decline, cancel) in [
            (false, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert!(Choice::parse(approval, decline, cancel, false).is_err());
        }
        for (approval, decline, cancel) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            assert!(Choice::parse(approval, decline, cancel, true).is_err());
        }
        let success = json!({"type":"commandExecution","status":"completed","exitCode":0,"aggregatedOutput":MARKER});
        let declined = json!({"type":"commandExecution","status":"declined","exitCode":null});
        assert!(Choice::Allow.matches(&success));
        assert!(!Choice::Allow.matches(&declined));
        assert!(Choice::Session.matches(&success));
        assert!(!Choice::Session.matches(&declined));
        assert!(Choice::Session.decision_matches(&Decision::Session {}));
        assert!(!Choice::Session.decision_matches(&Decision::Accept {}));
        assert!(!Choice::Allow.decision_matches(&Decision::Session {}));
        for choice in [Choice::Decline, Choice::Cancel] {
            assert!(choice.matches(&declined));
            assert!(!choice.matches(&success));
            assert!(
                !choice.matches(&json!({"type":"commandExecution","status":"failed","exitCode":1}))
            );
            assert!(!choice.matches(&json!({"type":"commandExecution","status":"inProgress"})));
        }
        assert!(Choice::Cancel.terminal("interrupted"));
        assert!(!Choice::Decline.terminal("interrupted"));
        assert!(!Choice::Allow.terminal("failed"));
    }
    #[test]
    fn approval_oracle_accepts_only_exact_print_and_known_no_profile_launcher() {
        let executables = [PathBuf::from(
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        )];
        let safe = format!(
            "\"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -NoProfile -Command \"{}\"",
            body()
        );
        assert!(print_only(&body(), &executables));
        assert!(print_only(&safe, &executables));
        let packaged = PathBuf::from(
            "C:\\Users\\fixture\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
        );
        let formatted = format!(
            "{} -NoProfile -Command \"{}\"",
            json!(packaged.to_str().unwrap()),
            body()
        );
        assert!(!print_only(&formatted, &executables));
        assert!(print_only(&formatted, &[packaged]));
        for unsafe_command in [
            format!("{}; whoami", body()),
            safe.replace(" -NoProfile", ""),
            safe.replace("C:\\Windows", "C:\\Downloads"),
            format!("{safe} && whoami"),
            "Write-Output NATIVE_APPROVAL_ALLOWED".into(),
        ] {
            assert!(!print_only(&unsafe_command, &executables));
        }
    }
}
