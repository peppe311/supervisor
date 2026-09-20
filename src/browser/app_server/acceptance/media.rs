//! Opt-in actual-host selected text/image acceptance. File picker UI is not driven:
//! only its result boundary is supplied with exact newly created fixture paths.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
const PROMPT: &str = "Use only the two attached inputs. Do not use tools, read filesystem paths or change anything. Reply in exactly two lines: TOKEN=<the token written in the attached note> and COLOR=<the solid image color in lowercase English>.";
const DRAFT: &str = "New unsent draft must survive attachment acceptance";
const SIBLING: &str = "Graph B keeps its independent unsent draft";
const LATE: &str = "later.txt";
const WARMUP: &str = "Without tools, file reads or changes, print the integers from 1 through 400, one per line, without skipping any. This is a streaming test; a follow-up may replace this instruction.";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Start,
    Queue,
    Steer,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Format {
    #[default]
    Jpeg,
    Png,
}
impl Format {
    fn suffix(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
        }
    }
}
pub(super) fn options(
    files: bool,
    png: bool,
    queue: bool,
    steer: bool,
) -> anyhow::Result<(Format, Mode)> {
    anyhow::ensure!(
        files || !(png || queue || steer),
        "--files-png/--files-queue/--files-steer require --files"
    );
    anyhow::ensure!(!(queue && steer), "Choose only one media delivery mode");
    Ok((
        if png { Format::Png } else { Format::Jpeg },
        if queue {
            Mode::Queue
        } else if steer {
            Mode::Steer
        } else {
            Mode::Start
        },
    ))
}
fn root_script(owner: &str) -> String {
    owner.strip_prefix("graph:").map_or_else(||"const root=document;".into(),|key|format!("const root=[...document.querySelectorAll('.agent-console[data-node-key]')].find(card=>card.dataset.nodeKey==={});",json!(key)))
}
fn script(app: &BrowserApp, owner: &str, body: &str) -> anyhow::Result<()> {
    let panel = if owner.starts_with("graph:") {
        &app.agent_graph_surface
    } else {
        &app.agent_panel
    };
    panel.as_ref().context("Missing media test surface")?.evaluate_script(&format!("try{{{} {body}}}catch(error){{(window.__nativeAcceptanceErrors||=[]).push(String(error));}}",root_script(owner)))?;
    Ok(())
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{{}const draft=root?.querySelector('textarea[data-role="message"],#chat-input');return {{
      renderer:typeof window.{},ready:!!draft,draft:draft?.value,delivery:{},
      files:[...root?.querySelectorAll('.file-context-card .context-title,ul[aria-label="Files attached to this draft"] strong')||[]].map(node=>node.textContent),
      previews:[...root?.querySelectorAll('.file-context-card img,ul[aria-label="Files attached to this draft"] img')||[]].filter(img=>img.complete&&img.naturalWidth>0).length,
      imageRecords:root?.querySelectorAll('.chat-message.user .message-file-attachment,.chat-message.user ul[aria-label="Attached files"] li').length||0,
      text:[...root?.querySelectorAll('.chat-message.assistant:not(.reasoning):not(.activity)')||[]].map(node=>node.textContent).join('\n'),
      siblings:[...document.querySelectorAll('.agent-console[data-node-key]')].filter(card=>card!==root).map(card=>({{draft:card.querySelector('textarea[data-role="message"]')?.value,files:card.querySelectorAll('ul[aria-label="Files attached to this draft"] li').length,active:card.dataset.assignmentBusy}})),
      errors:window.__nativeAcceptanceErrors||[]}};}})()"#,
        root_script(owner),
        if owner.starts_with("graph:") {
            "renderAgentGraphSurfaceState"
        } else {
            "renderAgentPanelState"
        },
        delivery::sample(owner)
    )
}
fn input(app: &BrowserApp, owner: &str, text: &str, submit: bool) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const input=root.querySelector('textarea[data-role=\"message\"],#chat-input');if(!input||input.disabled)throw new Error('Media composer unavailable');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));if({submit})input.dispatchEvent(new KeyboardEvent('keydown',{{key:'Enter',bubbles:true,cancelable:true}}));",
            json!(text)
        ),
    )
}
fn attach(app: &mut BrowserApp, owner: &str, paths: Vec<PathBuf>) -> anyhow::Result<()> {
    anyhow::ensure!(
        app.native_files_enabled(owner),
        "Native selected-file scope is unavailable"
    );
    if owner.starts_with("graph:") {
        app.graph_files
            .add_paths(owner, paths)
            .map_err(anyhow::Error::msg)?;
        app.render_agent_graph_surface();
    } else {
        anyhow::ensure!(app.composer_owner() == owner, "Wrong main attachment owner");
        app.add_chat_file_paths(paths);
    }
    Ok(())
}
fn encoded_image(color: [u8; 3], format: Format) -> anyhow::Result<Vec<u8>> {
    let image = image::RgbImage::from_pixel(64, 64, image::Rgb(color));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(
        &mut out,
        match format {
            Format::Jpeg => image::ImageFormat::Jpeg,
            Format::Png => image::ImageFormat::Png,
        },
    )?;
    Ok(out.into_inner())
}
fn answer_matches(text: &str, token: &str, color: &str) -> bool {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    text.contains(&format!("TOKEN={token}"))
        && text
            .to_ascii_lowercase()
            .contains(&format!("color={color}"))
}
#[derive(Default)]
pub(super) struct Media {
    format: Format,
    mode: Mode,
    starts: usize,
    first_turn: Option<String>,
    queue_observed: bool,
    phase: u8,
    opened: bool,
    thread: Option<String>,
    directory: Option<PathBuf>,
    token: String,
    color: &'static str,
    image_url: String,
    selected: Vec<String>,
    later_id: Option<String>,
    accepted: bool,
    read: bool,
    error: Option<String>,
}
impl Media {
    pub(super) fn new(format: Format, mode: Mode) -> Self {
        Self {
            format,
            mode,
            ..Self::default()
        }
    }
    fn select_fixture(&mut self, app: &mut BrowserApp, owner: &str) -> anyhow::Result<()> {
        let directory = self
            .directory
            .as_ref()
            .context("Missing fixture directory")?;
        self.token = format!("NATIVE_FILE_{}", Uuid::new_v4().simple());
        let colors = [
            ("red", [255, 0, 0]),
            ("green", [0, 255, 0]),
            ("blue", [0, 0, 255]),
        ];
        let (color, rgb) = colors[Uuid::new_v4().as_bytes()[0] as usize % colors.len()];
        self.color = color;
        let bytes = encoded_image(rgb, self.format)?;
        self.image_url = format!(
            "data:image/{};base64,{}",
            self.format.suffix(),
            STANDARD.encode(&bytes)
        );
        let note = directory.join("note.txt");
        let picture = directory.join(format!("image.{}", self.format.suffix()));
        fs::write(&note, &self.token)?;
        fs::write(&picture, &bytes)?;
        attach(app, owner, vec![note.clone(), picture.clone()])?;
        self.selected = if owner.starts_with("graph:") {
            app.graph_files
                .views(owner)
                .iter()
                .map(|file| file.id.clone())
                .collect()
        } else {
            app.draft_file_attachments
                .iter()
                .map(|file| file.id().into())
                .collect()
        };
        anyhow::ensure!(self.selected.len() == 2, "Fixture files were not selected");
        fs::write(note, "NEW_DISK_CONTENT_MUST_NOT_BE_SENT")?;
        fs::write(picture, encoded_image([0, 0, 0], self.format)?)?;
        Ok(())
    }
    fn retain_later_draft(&mut self, app: &mut BrowserApp, owner: &str) -> anyhow::Result<()> {
        if self.later_id.is_some() {
            return Ok(());
        }
        let late = self
            .directory
            .as_ref()
            .context("No fixture directory")?
            .join(LATE);
        fs::write(&late, "Unsent file belongs to the next draft")?;
        attach(app, owner, vec![late])?;
        self.later_id = if owner.starts_with("graph:") {
            app.graph_files
                .views(owner)
                .iter()
                .find(|file| file.name == LATE)
                .map(|file| file.id.clone())
        } else {
            app.draft_file_attachments
                .iter()
                .find(|file| file.content().name == LATE)
                .map(|file| file.id().into())
        };
        anyhow::ensure!(self.later_id.is_some(), "Later attachment was not loaded");
        input(app, owner, DRAFT, false)
    }
    pub(super) fn observe(&mut self, app: &mut BrowserApp, owner: &str, event: &BrowserEvent) {
        let result = (|| -> anyhow::Result<()> {
            let BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) = event
            else {
                return Ok(());
            };
            if request.owner != owner {
                return Ok(());
            }
            let value = result.as_ref().map_err(|error| {
                anyhow::anyhow!("Native {} failed: {error}", request.call.method)
            })?;
            if request.call.method == "turn/start" {
                self.starts += 1;
                if request.call.params["input"][0]["text"] == WARMUP {
                    anyhow::ensure!(
                        self.mode != Mode::Start
                            && self.starts == 1
                            && request.call.params["input"]
                                .as_array()
                                .is_some_and(|items| items.len() == 1),
                        "Unexpected warmup input"
                    );
                    self.first_turn = Some(
                        value["turn"]["id"]
                            .as_str()
                            .context("Warmup turn ID missing")?
                            .into(),
                    );
                    return Ok(());
                }
            }
            if matches!(request.call.method, "turn/start" | "turn/steer") {
                anyhow::ensure!(
                    request.call.method
                        == if self.mode == Mode::Steer {
                            "turn/steer"
                        } else {
                            "turn/start"
                        },
                    "Media used the wrong delivery API"
                );
                if self.mode == Mode::Steer {
                    anyhow::ensure!(
                        self.first_turn.is_some()
                            && request.call.params["expectedTurnId"].as_str()
                                == self.first_turn.as_deref()
                            && value["turnId"].as_str() == self.first_turn.as_deref(),
                        "Media steering targeted another turn"
                    );
                }
                anyhow::ensure!(!self.accepted, "Media input was sent more than once");
                let items = request.call.params["input"]
                    .as_array()
                    .context("Missing native file input")?;
                anyhow::ensure!(
                    items.len() == 4
                        && items[0]["text"] == PROMPT
                        && items[1]["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(&self.token)
                                && !text.contains("NEW_DISK_CONTENT"))
                        && items[3]["type"] == "image"
                        && items[3]["url"] == self.image_url,
                    "Native input did not preserve selected text and image snapshots"
                );
                // Arrival before production ACK cleanup deliberately exercises the race.
                self.retain_later_draft(app, owner)?;
                self.accepted = true;
            }
            if request.call.method == "thread/read" {
                let cwd = value["thread"]["cwd"]
                    .as_str()
                    .and_then(|path| fs::canonicalize(path).ok());
                anyhow::ensure!(
                    cwd.is_some()
                        && cwd
                            == self
                                .directory
                                .as_ref()
                                .and_then(|path| fs::canonicalize(path).ok()),
                    "Media readback directory changed"
                );
                self.read = true;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error.to_string());
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
        anyhow::ensure!(sample["renderer"] == "function", "Media renderer missing");
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            if let Some(graph) = graph {
                if !self.opened {
                    graph.open_cards(app)?;
                    self.opened = true;
                    return Ok(false);
                }
                if sample["ready"] != true
                    || sample["siblings"].as_array().is_none_or(|v| v.len() != 1)
                {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    &format!(
                        "for(const card of document.querySelectorAll('.agent-console[data-node-key]')){{if(card!==root){{const input=card.querySelector('textarea[data-role=\"message\"]');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));}}}}",
                        json!(SIBLING)
                    ),
                )?;
            }
            let directory = graph
                .and_then(|graph| graph.directory(owner))
                .or_else(|| app.workspace.root())
                .context("Missing media fixture workspace")?
                .to_path_buf();
            self.directory = Some(directory);
            if self.mode != Mode::Start {
                if graph.is_some() {
                    script(
                        app,
                        owner,
                        "root.addEventListener('central-agent:graph-composer-state',event=>root.__nativeDeliveryState=event.detail);",
                    )?;
                }
                input(app, owner, WARMUP, true)?;
                self.phase = 10;
                return Ok(false);
            }
            self.select_fixture(app, owner)?;
            self.phase = 1;
            return Ok(false);
        }
        if self.phase == 10 {
            let active = app.app_server.conversations.active_turn(owner);
            if self.first_turn.is_none()
                || active != self.first_turn.as_deref()
                || sample["delivery"]["turn"].as_str() != active
                || sample["delivery"]["steer"] != "true"
                || sample["draft"] != ""
            {
                return Ok(false);
            }
            self.select_fixture(app, owner)?;
            self.phase = 1;
            return Ok(false);
        }
        if self.phase == 1 {
            if sample["files"]
                .as_array()
                .is_none_or(|files| files.len() != 2)
                || sample["previews"] != 1
            {
                return Ok(false);
            }
            input(app, owner, PROMPT, true)?;
            self.phase = if self.mode == Mode::Start { 2 } else { 11 };
            println!(
                "Media host: decoded selected {:?} preview and two draft files; submitted through actual composer Enter ({:?}).",
                self.format, self.mode
            );
            return Ok(false);
        }
        if self.phase == 11 {
            anyhow::ensure!(
                app.app_server.conversations.active_turn(owner) == self.first_turn.as_deref(),
                "Warmup ended before concurrent media delivery"
            );
            if sample["delivery"]["choices"] != true {
                return Ok(false);
            }
            delivery::click(
                app,
                owner,
                if self.mode == Mode::Queue {
                    "agent-delivery-queue"
                } else {
                    "agent-delivery-now"
                },
            )?;
            self.phase = if self.mode == Mode::Queue { 12 } else { 2 };
            return Ok(false);
        }
        if self.phase == 12 {
            if !app
                .agent_submission_queue
                .iter()
                .any(|entry| entry.owner() == Some(owner) && entry.message == PROMPT)
            {
                return Ok(false);
            }
            anyhow::ensure!(
                self.starts == 1
                    && app.app_server.conversations.active_turn(owner)
                        == self.first_turn.as_deref(),
                "Media did not remain queued behind the active turn"
            );
            self.queue_observed = true;
            self.retain_later_draft(app, owner)?;
            self.phase = 2;
            println!("Media host: held queue observed before dequeue; newer draft/file installed.");
            return Ok(false);
        }
        if graph.is_some() {
            let siblings = sample["siblings"]
                .as_array()
                .context("Missing media sibling sample")?;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["draft"] == SIBLING
                    && siblings[0]["files"] == 0
                    && siblings[0]["active"] != "true",
                "Media affected sibling graph draft/files"
            );
            anyhow::ensure!(
                app.app_server
                    .conversations
                    .saved()
                    .bindings
                    .keys()
                    .all(|key| key == owner),
                "Media created another native conversation"
            );
        }
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        if let Some(thread) = &self.thread {
            anyhow::ensure!(
                *thread == binding.thread_id,
                "Media replaced native history"
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
            thread.turns.len() <= if self.mode == Mode::Queue { 2 } else { 1 }
                && thread.turns.iter().all(|turn| turn.status != "failed"),
            "Native media turn failed or was duplicated"
        );
        anyhow::ensure!(
            thread
                .turns
                .iter()
                .flat_map(|turn| &turn.items)
                .all(|item| matches!(
                    item.value["type"].as_str(),
                    Some("userMessage" | "agentMessage" | "reasoning" | "plan")
                )),
            "Media test used an unexpected tool"
        );
        if !self.accepted
            || thread.turns.is_empty()
            || thread.turns.iter().any(|turn| turn.status != "completed")
            || app.app_server.busy(owner)
        {
            return Ok(false);
        }
        let draft_files = if graph.is_some() {
            app.graph_files.views(owner)
        } else {
            app.draft_file_attachments
                .iter()
                .map(PendingFileAttachment::view)
                .collect()
        };
        anyhow::ensure!(
            draft_files.len() == 1
                && Some(draft_files[0].id.as_str()) == self.later_id.as_deref()
                && !self.selected.contains(&draft_files[0].id),
            "Acceptance consumed newer file or retained sent files"
        );
        if sample["draft"] != DRAFT
            || sample["files"] != json!([LATE])
            || sample["imageRecords"] != 1
        {
            return Ok(false);
        }
        anyhow::ensure!(
            self.starts == if self.mode == Mode::Queue { 2 } else { 1 },
            "Unexpected number of native media turns"
        );
        anyhow::ensure!(
            self.mode != Mode::Queue || self.queue_observed,
            "Media queue was never observed before dequeue"
        );
        let media_turn = thread.turns.last().context("Missing media turn")?;
        let native_answer = media_turn
            .items
            .iter()
            .filter(|item| item.value["type"] == "agentMessage")
            .filter_map(|item| item.value["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::ensure!(
            answer_matches(&native_answer, &self.token, self.color),
            "Model did not recognize original attached text/image: {native_answer}"
        );
        // The authoritative completed item can arrive before the next DOM sample.
        if !answer_matches(
            sample["text"].as_str().unwrap_or_default(),
            &self.token,
            self.color,
        ) {
            return Ok(false);
        }
        let content: Vec<_> = media_turn
            .items
            .iter()
            .filter(|item| item.value["type"] == "userMessage")
            .filter_map(|item| item.value["content"].as_array())
            .flatten()
            .collect();
        anyhow::ensure!(
            content.iter().any(|item| item["text"]
                .as_str()
                .is_some_and(|text| text.contains(&self.token)))
                && content
                    .iter()
                    .any(|item| matches!(item["type"].as_str(), Some("image" | "localImage"))),
            "Native history lost selected text/image content"
        );
        if self.phase == 2 {
            app.app_server_conversation(owner.into(), ConversationAction::Read {});
            self.phase = 3;
            return Ok(false);
        }
        if self.read {
            anyhow::ensure!(
                app.app_server.conversations.saved().unresolved.is_empty()
                    && app
                        .project_chats
                        .iter()
                        .all(|chat| chat.messages.is_empty()),
                "Media left uncertain input or copied other-provider history"
            );
            println!(
                "Media host: original text/image recognized; later file/draft survived ACK; native image record and same-directory readback verified."
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
    fn fixture_is_decodable_and_answer_needs_both_independent_inputs() {
        for format in [Format::Jpeg, Format::Png] {
            let bytes = encoded_image([0, 255, 0], format).unwrap();
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (64, 64));
            assert_eq!(
                image::guess_format(&bytes).unwrap(),
                if format == Format::Png {
                    image::ImageFormat::Png
                } else {
                    image::ImageFormat::Jpeg
                }
            );
        }
        assert!(answer_matches(
            "TOKEN=fixture\nCOLOR=green",
            "fixture",
            "green"
        ));
        assert!(!answer_matches(
            "TOKEN=fixture\nCOLOR=blue",
            "fixture",
            "green"
        ));
        assert!(!answer_matches(
            "TOKEN=other\nCOLOR=green",
            "fixture",
            "green"
        ));
    }
    #[test]
    fn media_options_require_explicit_files_and_exactly_one_delivery_mode() {
        assert_eq!(
            options(true, true, false, false).unwrap(),
            (Format::Png, Mode::Start)
        );
        assert_eq!(
            options(true, false, true, false).unwrap(),
            (Format::Jpeg, Mode::Queue)
        );
        assert_eq!(
            options(true, true, false, true).unwrap(),
            (Format::Png, Mode::Steer)
        );
        assert!(options(true, true, true, true).is_err());
        for (png, queue, steer) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert!(options(false, png, queue, steer).is_err());
        }
    }
}
