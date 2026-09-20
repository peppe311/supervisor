//! Exact disposable-file approval oracle, never a general patch executor.
use super::*;

pub(super) const NAME: &str = "native-approval.txt";
pub(super) const BEFORE: &str = "NATIVE_FILE_BEFORE";
pub(super) const AFTER: &str = "NATIVE_FILE_AFTER";

pub(super) fn prepare(directory: &Path) -> anyhow::Result<String> {
    let path = directory.join(NAME);
    anyhow::ensure!(!path.exists(), "Refusing to replace an existing test file");
    fs::write(&path, format!("{BEFORE}\n"))?;
    Ok(format!(
        "Use the native apply_patch tool exactly once to replace the sole line {BEFORE} with {AFTER} in {}. That existing file contains exactly {BEFORE} followed by a newline. Do not use shell commands or any other tools, read any other files, make other changes, request extra permissions or change configuration. Let the native file-change approval ask the user. If declined or cancelled, do not retry. After successful application reply with {AFTER}; otherwise report that the change was rejected.",
        path.display()
    ))
}

fn exact_diff(diff: &str) -> bool {
    let changed = diff
        .lines()
        .filter(|line| {
            (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"))
        })
        .collect::<Vec<_>>();
    changed == [format!("-{BEFORE}"), format!("+{AFTER}")]
}

pub(super) fn validate(directory: &Path, item: &Value, params: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        item["type"] == "fileChange",
        "No native file-change preview; nothing approved"
    );
    let changes = item["changes"]
        .as_array()
        .context("Missing proposed changes")?;
    anyhow::ensure!(
        changes.len() == 1,
        "Only the one disposable file may be approved"
    );
    let change = &changes[0];
    let path = PathBuf::from(change["path"].as_str().context("Missing changed path")?);
    anyhow::ensure!(
        !path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir)),
        "Parent traversal in patch path"
    );
    let path = if path.is_absolute() {
        path
    } else {
        directory.join(path)
    };
    anyhow::ensure!(
        fs::canonicalize(path)? == fs::canonicalize(directory.join(NAME))?,
        "Patch targeted another file"
    );
    anyhow::ensure!(
        change["kind"]["type"] == "update" && change["kind"]["move_path"].is_null(),
        "Patch is not an in-place update"
    );
    anyhow::ensure!(
        change["diff"].as_str().is_some_and(exact_diff),
        "Patch differs from the exact one-line replacement: {}",
        change["diff"]
    );
    if let Some(root) = params["grantRoot"].as_str() {
        anyhow::ensure!(
            fs::canonicalize(root)? == fs::canonicalize(directory)?,
            "Unexpected requested write root"
        );
    }
    Ok(())
}

pub(super) fn disk(directory: &Path, accepted: bool) -> anyhow::Result<()> {
    let paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    anyhow::ensure!(
        paths.len() == 1 && paths[0] == NAME,
        "File test changed other workspace paths"
    );
    let content = fs::read_to_string(directory.join(NAME))?;
    let expected = if accepted { AFTER } else { BEFORE };
    anyhow::ensure!(
        content == format!("{expected}\n") || content == format!("{expected}\r\n"),
        "Unexpected actual file contents: {content:?}"
    );
    Ok(())
}

pub(super) fn outcome(item: &Value, accepted: bool) -> bool {
    item["type"] == "fileChange"
        && item["status"] == if accepted { "completed" } else { "declined" }
}

// This is only one part of the Cancel oracle: the caller also requires the
// successful actual Cancel answer, resolved request, exact unchanged file,
// no additional tool work, and authoritative thread/read confirmation.
pub(super) fn cancelled(item: &Value, turn_status: &str) -> bool {
    turn_status == "interrupted"
        && item["type"] == "fileChange"
        && matches!(
            item["status"].as_str(),
            Some("inProgress" | "failed" | "declined")
        )
}

pub(super) fn rendered(text: &Value) -> bool {
    text.as_str()
        .is_some_and(|text| text.contains(BEFORE) && text.contains(AFTER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_approval_rejects_broader_changes_paths_and_moves() {
        let directory = tempfile::tempdir().unwrap();
        prepare(directory.path()).unwrap();
        let diff = format!("--- a/{NAME}\n+++ b/{NAME}\n@@ -1 +1 @@\n-{BEFORE}\n+{AFTER}\n");
        let item = json!({"type":"fileChange","status":"inProgress","changes":[{"path":directory.path().join(NAME),"kind":{"type":"update","move_path":null},"diff":diff}]});
        assert!(validate(directory.path(), &item, &json!({})).is_ok());
        let mut wrong = item.clone();
        wrong["changes"][0]["diff"] = json!(format!("{diff}+extra\n"));
        assert!(validate(directory.path(), &wrong, &json!({})).is_err());
        wrong = item.clone();
        wrong["changes"][0]["kind"]["move_path"] = json!("other.txt");
        assert!(validate(directory.path(), &wrong, &json!({})).is_err());
        wrong = item.clone();
        wrong["changes"][0]["path"] = json!("../native-approval.txt");
        assert!(validate(directory.path(), &wrong, &json!({})).is_err());
        wrong = item.clone();
        wrong["changes"]
            .as_array_mut()
            .unwrap()
            .push(item["changes"][0].clone());
        assert!(validate(directory.path(), &wrong, &json!({})).is_err());
        disk(directory.path(), false).unwrap();
        assert!(disk(directory.path(), true).is_err());
    }

    #[test]
    fn file_rejection_is_not_execution_failure_or_success() {
        let declined = json!({"type":"fileChange","status":"declined"});
        let completed = json!({"type":"fileChange","status":"completed"});
        assert!(outcome(&declined, false));
        assert!(!outcome(&declined, true));
        assert!(outcome(&completed, true));
        assert!(!outcome(&completed, false));
        assert!(!outcome(
            &json!({"type":"fileChange","status":"failed"}),
            false
        ));
        assert!(!outcome(
            &json!({"type":"commandExecution","status":"completed"}),
            true
        ));
        assert!(!exact_diff(&format!("+{AFTER}\n")));
    }

    #[test]
    fn file_cancel_requires_interrupted_turn_and_never_accepts_applied_changes() {
        for status in ["inProgress", "failed", "declined"] {
            let item = json!({"type":"fileChange","status":status});
            assert!(cancelled(&item, "interrupted"));
            for turn in ["inProgress", "completed", "failed", ""] {
                assert!(!cancelled(&item, turn));
            }
        }
        assert!(!cancelled(
            &json!({"type":"fileChange","status":"completed"}),
            "interrupted"
        ));
        assert!(!cancelled(
            &json!({"type":"commandExecution","status":"declined"}),
            "interrupted"
        ));
    }
}
