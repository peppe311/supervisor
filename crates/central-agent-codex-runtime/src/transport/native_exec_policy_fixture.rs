// Shared deterministic acceptance fixture, never ordinary model or tool execution.
use super::reload_responses_fixture;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) const MARKER: &str = "CENTRAL_NATIVE_POLICY_ALLOWED";
pub(super) const INPUT: &str = "NATIVE_EXEC_POLICY_FIXTURE";

pub(super) fn expression() -> String {
    format!("[string]::Concat('{MARKER}')")
}
pub(super) fn shell() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system directory"))
        .join("System32/WindowsPowerShell/v1.0/powershell.exe")
}
pub(super) fn prefix() -> Value {
    json!([shell(), "-NoProfile", "-Command", expression()])
}
pub(super) fn owned_policy(params: &Value) -> bool {
    let Some(parts) = params["proposedExecpolicyAmendment"].as_array() else {
        return false;
    };
    if parts.len() != 4
        || parts[1] != "-NoProfile"
        || parts[2] != "-Command"
        || parts[3] != expression()
    {
        return false;
    }
    let Some(executable) = parts[0].as_str() else {
        return false;
    };
    let bundled = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(
        ".cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    );
    let actual = Path::new(executable).canonicalize();
    if ![shell(), bundled].iter().any(|p| {
        actual
            .as_ref()
            .ok()
            .is_some_and(|a| p.canonicalize().as_ref().ok() == Some(a))
    }) {
        return false;
    }
    // The native runtime may select its bundled PowerShell. Accept only that
    // known binary and the entire literal expression, never a general shell rule.
    let commands = [
        executable.to_owned(),
        format!("\"{executable}\""),
        json!(executable).to_string(),
    ];
    commands
        .iter()
        .any(|exe| params["command"] == format!("{exe} -NoProfile -Command \"{}\"", expression()))
        && params["commandActions"] == json!([{"type":"unknown","command":expression()}])
}
pub(super) fn model_output(body: &Value, index: usize) -> std::result::Result<Value, String> {
    let input = body["input"].as_array().ok_or("Missing fixture input")?;
    let last = input
        .iter()
        .rposition(|i| i["role"] == "user")
        .ok_or("Missing user")?;
    if !input[last].to_string().contains(INPUT) {
        return Err("Unknown print fixture input".into());
    }
    if input[last + 1..]
        .iter()
        .any(|i| i["type"] == "custom_tool_call_output" || i["type"] == "function_call_output")
    {
        return reload_responses_fixture::ready_output(body, index);
    }
    let tools = body["tools"]
        .as_array()
        .or_else(|| {
            input
                .iter()
                .find(|i| i["type"] == "additional_tools")
                .and_then(|i| i["tools"].as_array())
        })
        .ok_or("Missing native tools")?;
    if !tools.iter().any(|t| {
        t["type"] == "namespace"
            && t["name"] == "functions"
            && t["tools"].as_array().is_some_and(|ts| {
                ts.iter()
                    .any(|t| t["type"] == "custom" && t["name"] == "exec")
            })
    }) {
        return Err("Native code-mode tool missing".into());
    }
    let args = json!({"cmd":expression(),"shell":shell(),"login":false,
        "sandbox_permissions":"require_escalated",
        "justification":"Allow this exact owned print-only policy fixture?",
        "prefix_rule":prefix()});
    Ok(
        json!({"id":format!("function-{index}"),"type":"custom_tool_call",
        "call_id":format!("policy-call-{index}"),"name":"exec","namespace":"functions",
        "input":format!("text(await tools.exec_command({args}));"),"status":"completed"}),
    )
}
