//! Read-only Codex Cloud discovery through the official subscription CLI.
//!
//! The CLI owns authentication and cloud transport. Supervisor never reads an
//! access token, calls a private endpoint, or projects a cloud task as a local
//! App Server thread.
use crate::runtime::Runtime;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    process::{Command, ExitStatus, Stdio},
    thread,
};
use url::Url;

const MAX_LIST_BYTES: usize = 1024 * 1024;
const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 16 * 1024;
const MAX_SAFE_UI_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    #[serde(rename(deserialize = "files_changed", serialize = "filesChanged"))]
    pub files_changed: u64,
    #[serde(rename(deserialize = "lines_added", serialize = "linesAdded"))]
    pub lines_added: u64,
    #[serde(rename(deserialize = "lines_removed", serialize = "linesRemoved"))]
    pub lines_removed: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    #[serde(skip_serializing)]
    pub url: String,
    pub title: String,
    pub status: String,
    #[serde(rename(deserialize = "updated_at", serialize = "updatedAt"))]
    pub updated_at: String,
    #[serde(rename(deserialize = "environment_id", serialize = "environmentId"))]
    #[serde(skip_serializing)]
    pub environment_id: Option<String>,
    #[serde(rename(deserialize = "environment_label", serialize = "environmentLabel"))]
    pub environment_label: String,
    pub summary: TaskSummary,
    #[serde(rename(deserialize = "is_review", serialize = "isReview"))]
    pub is_review: bool,
    #[serde(rename(deserialize = "attempt_total", serialize = "attemptTotal"))]
    pub attempt_total: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TaskPage {
    pub tasks: Vec<Task>,
    pub cursor: Option<String>,
}

impl Runtime {
    /// Lists cloud tasks belonging to the ChatGPT account signed into this
    /// runtime's isolated profile. This uses subscription authentication, not
    /// an OpenAI API key.
    pub fn list_cloud_tasks(&self, cursor: Option<&str>, limit: u8) -> Result<TaskPage, String> {
        if !(1..=20).contains(&limit) {
            return Err("Codex Cloud accepts between 1 and 20 tasks per page".into());
        }
        if let Some(cursor) = cursor {
            validate_cursor(cursor)?;
        }
        let mut command = self.scoped_command();
        let limit = limit.to_string();
        command.args(["cloud", "list", "--json", "--limit", &limit]);
        if let Some(cursor) = cursor {
            command.args(["--cursor", cursor]);
        }
        let output = run_bounded(command, MAX_LIST_BYTES, "Codex Cloud task list")?;
        let mut page: TaskPage = serde_json::from_slice(&output)
            .map_err(|_| "Codex returned an invalid Cloud task list".to_owned())?;
        validate_page(&mut page)?;
        Ok(page)
    }

    /// Reads the official unified diff for one already observed cloud task.
    pub fn cloud_task_diff(&self, task_id: &str, attempt: u32) -> Result<String, String> {
        validate_task_id(task_id)?;
        if !(1..=64).contains(&attempt) {
            return Err("Select an available Codex Cloud attempt".into());
        }
        let mut command = self.scoped_command();
        let attempt = attempt.to_string();
        command.args(["cloud", "diff", task_id, "--attempt", &attempt]);
        let output = run_bounded(command, MAX_DIFF_BYTES, "Codex Cloud diff")?;
        String::from_utf8(output).map_err(|_| "Codex returned a non-text Cloud diff".into())
    }
}

fn validate_page(page: &mut TaskPage) -> Result<(), String> {
    if page.tasks.len() > 20 {
        return Err("Codex returned too many Cloud tasks".into());
    }
    if let Some(cursor) = page.cursor.as_deref() {
        validate_cursor(cursor)?;
    }
    for task in &mut page.tasks {
        validate_task_id(&task.id)?;
        task.url = validate_task_url(&task.url, &task.id)?;
        task.title = compact_text(&task.title, 240, "Cloud task title")?;
        task.status = compact_text(&task.status, 64, "Cloud task status")?;
        task.updated_at = compact_text(&task.updated_at, 96, "Cloud task timestamp")?;
        task.environment_label =
            compact_text(&task.environment_label, 160, "Cloud environment label")?;
        if let Some(environment_id) = task.environment_id.as_mut() {
            *environment_id = compact_text(environment_id, 256, "Cloud environment identity")?;
        }
        if task.attempt_total > 64 {
            return Err("Codex returned an invalid Cloud attempt count".into());
        }
        if [
            task.summary.files_changed,
            task.summary.lines_added,
            task.summary.lines_removed,
        ]
        .into_iter()
        .any(|value| value > MAX_SAFE_UI_INTEGER)
        {
            return Err("Codex returned invalid Cloud change totals".into());
        }
    }
    Ok(())
}

fn validate_task_id(value: &str) -> Result<(), String> {
    if value.len() < 8
        || value.len() > 160
        || !value.starts_with("task_")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("Codex returned an invalid Cloud task identity".into());
    }
    Ok(())
}

fn validate_task_url(value: &str, task_id: &str) -> Result<String, String> {
    let parsed = Url::parse(value).map_err(|_| "Codex returned an invalid Cloud task URL")?;
    let expected_path = format!("/codex/tasks/{task_id}");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("chatgpt.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != expected_path
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("Codex returned an unexpected Cloud task URL".into());
    }
    Ok(parsed.to_string())
}

fn validate_cursor(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(|character| character.is_control())
    {
        return Err("Codex returned an invalid Cloud pagination cursor".into());
    }
    Ok(())
}

fn compact_text(value: &str, maximum: usize, label: &str) -> Result<String, String> {
    if value.chars().any(|character| character == '\0') {
        return Err(format!("Codex returned an invalid {label}"));
    }
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return Err(format!("Codex returned an invalid {label}"));
    }
    let mut characters = compact.chars();
    let mut limited = characters.by_ref().take(maximum).collect::<String>();
    if characters.next().is_some() {
        limited.pop();
        limited.push('…');
    }
    Ok(limited)
}

struct Captured {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn drain(mut reader: impl Read, maximum: usize) -> std::io::Result<Captured> {
    let mut bytes = Vec::with_capacity(maximum.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    Ok(Captured { bytes, exceeded })
}

fn run_bounded(mut command: Command, maximum: usize, label: &str) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|_| format!("Could not start the official {label} command"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Could not read the official {label} command"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Could not read the official {label} command"))?;
    let output = thread::spawn(move || drain(stdout, maximum));
    let errors = thread::spawn(move || drain(stderr, MAX_ERROR_BYTES));
    let status = child
        .wait()
        .map_err(|_| format!("The official {label} command did not finish correctly"))?;
    let output = output
        .join()
        .map_err(|_| format!("The official {label} output could not be read"))?
        .map_err(|_| format!("The official {label} output could not be read"))?;
    let errors = errors
        .join()
        .map_err(|_| format!("The official {label} error could not be read"))?
        .map_err(|_| format!("The official {label} error could not be read"))?;
    if output.exceeded {
        return Err(format!("The {label} exceeds Supervisor's display limit"));
    }
    if !status.success() {
        return Err(command_failure(label, status, &errors.bytes));
    }
    Ok(output.bytes)
}

fn command_failure(label: &str, status: ExitStatus, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr)
        .split_whitespace()
        .take(48)
        .collect::<Vec<_>>()
        .join(" ");
    if detail.is_empty() {
        format!("The official {label} command failed ({status})")
    } else {
        format!("The official {label} command failed: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn validates_and_compacts_official_cloud_page() {
        let mut page: TaskPage = serde_json::from_value(serde_json::json!({
            "tasks":[{
                "id":"task_e_fixture123",
                "url":"https://chatgpt.com/codex/tasks/task_e_fixture123",
                "title":"  Fix   the project  ",
                "status":"ready",
                "updated_at":"2026-09-17T10:00:00Z",
                "environment_id":null,
                "environment_label":"Supervisor",
                "summary":{"files_changed":2,"lines_added":4,"lines_removed":1},
                "is_review":false,
                "attempt_total":1
            }],
            "cursor":null
        }))
        .unwrap();
        validate_page(&mut page).unwrap();
        assert_eq!(page.tasks[0].title, "Fix the project");
        let projected = serde_json::to_string(&page.tasks[0]).unwrap();
        assert!(!projected.contains("chatgpt.com") && !projected.contains("environmentId"));
        assert!(projected.contains("filesChanged") && projected.contains("updatedAt"));
    }

    #[test]
    fn rejects_untrusted_cloud_identity_and_url() {
        for (id, url) in [
            (
                "thread_123456",
                "https://chatgpt.com/codex/tasks/thread_123456",
            ),
            (
                "task_e_fixture123",
                "https://example.com/codex/tasks/task_e_fixture123",
            ),
            ("task_e_fixture123", "https://chatgpt.com/codex/tasks/other"),
        ] {
            let mut page: TaskPage = serde_json::from_value(serde_json::json!({
                "tasks":[{"id":id,"url":url,"title":"Task","status":"ready","updated_at":"now","environment_id":null,"environment_label":"Environment","summary":{"files_changed":0,"lines_added":0,"lines_removed":0},"is_review":false,"attempt_total":1}],
                "cursor":null
            })).unwrap();
            assert!(validate_page(&mut page).is_err());
        }
    }

    #[test]
    fn bounded_reader_drains_without_growing_past_limit() {
        let input = vec![b'x'; 4096];
        let captured = drain(input.as_slice(), 128).unwrap();
        assert_eq!(captured.bytes.len(), 128);
        assert!(captured.exceeded);
    }

    #[test]
    fn command_failure_never_requires_raw_stderr() {
        let mut sink = Vec::new();
        write!(&mut sink, "not signed in; connect your ChatGPT account").unwrap();
        let status = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "exit", "1"])
                .status()
                .unwrap()
        } else {
            Command::new("sh").args(["-c", "exit 1"]).status().unwrap()
        };
        let message = command_failure("Codex Cloud", status, &sink);
        assert!(message.contains("ChatGPT account"));
    }
}
