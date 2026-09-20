//! Only an explicitly opted-in test's new empty temporary workspace is writable.
use super::*;
use std::process::Command;

const GOOD: &str = "/// Returns true exactly when n is a positive integer.\npub fn is_positive(n: i32) -> bool { n > 0 }\n";
const BAD: &str = "/// Returns true exactly when n is a positive integer.\npub fn is_positive(n: i32) -> bool { n < 0 }\n";
const MANIFEST: &str =
    "[package]\nname = \"native-review-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const INSTRUCTIONS: &str = "This is a disposable native code-review fixture. Review only this repository. Read the Git diff and src/lib.rs; the documented function contract is authoritative. Report findings without editing files, building, installing dependencies or accessing the network. All relevant code is here.\n";

pub(super) struct Fixture {
    pub target: Value,
    root: PathBuf,
    before: Vec<Vec<u8>>,
}

fn git(root: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "core.hooksPath=.git/disabled-fixture-hooks",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "user.name=Native review fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Disposable fixture Git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn snapshot(root: &Path) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut snapshot = Vec::new();
    for name in ["AGENTS.md", "Cargo.toml", "src/lib.rs"] {
        let path = root.join(name);
        anyhow::ensure!(
            fs::symlink_metadata(&path)?.file_type().is_file(),
            "Fixture path is not a regular file"
        );
        snapshot.push(fs::read(path)?);
    }
    for args in [
        vec!["rev-parse", "HEAD"],
        vec!["status", "--porcelain=v1", "--untracked-files=all"],
        vec!["diff", "--binary"],
        vec!["diff", "--cached", "--binary"],
        vec!["ls-files"],
    ] {
        snapshot.push(git(root, &args)?);
    }
    Ok(snapshot)
}

impl Fixture {
    pub(super) fn create(root: &Path, mode: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            ["uncommitted", "branch", "commit"].contains(&mode),
            "Unknown Git review fixture"
        );
        let root = fs::canonicalize(root)?;
        let temp = fs::canonicalize(std::env::temp_dir())?;
        anyhow::ensure!(
            root != temp && root.starts_with(&temp) && fs::read_dir(&root)?.next().is_none(),
            "Git review fixture requires its new empty temporary directory"
        );
        git(&root, &["init", "--quiet", "--initial-branch=fixture-base"])?;
        fs::write(root.join("AGENTS.md"), INSTRUCTIONS)?;
        fs::write(root.join("Cargo.toml"), MANIFEST)?;
        fs::create_dir(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), GOOD)?;
        git(
            &root,
            &["add", "--", "AGENTS.md", "Cargo.toml", "src/lib.rs"],
        )?;
        git(
            &root,
            &["commit", "--quiet", "-m", "Known positive-integer contract"],
        )?;
        git(&root, &["checkout", "--quiet", "-b", "fixture-change"])?;
        fs::write(root.join("src/lib.rs"), BAD)?;
        if mode != "uncommitted" {
            git(&root, &["add", "--", "src/lib.rs"])?;
            git(
                &root,
                &[
                    "commit",
                    "--quiet",
                    "-m",
                    "Invert positive-integer comparison",
                ],
            )?;
        }
        let target = match mode {
            "uncommitted" => json!({"type":"uncommittedChanges"}),
            "branch" => json!({"type":"baseBranch","branch":"fixture-base"}),
            _ => {
                json!({"type":"commit","sha":String::from_utf8(git(&root, &["rev-parse", "HEAD"])?)?.trim(),"title":null})
            }
        };
        let before = snapshot(&root)?;
        Ok(Self {
            target,
            root,
            before,
        })
    }
    pub(super) fn verify(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            snapshot(&self.root)? == self.before,
            "Review changed its fixture files, HEAD, index or worktree"
        );
        Ok(())
    }
    pub(super) fn owns(&self, cwd: &str) -> bool {
        fs::canonicalize(cwd).is_ok_and(|actual| actual == self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_builds_each_native_git_target_and_detects_mutation() {
        for mode in ["uncommitted", "branch", "commit"] {
            let dir = tempfile::tempdir().unwrap();
            let fixture = Fixture::create(dir.path(), mode).unwrap();
            fixture.verify().unwrap();
            assert!(fixture.owns(dir.path().to_str().unwrap()));
            assert!(!fixture.owns(dir.path().parent().unwrap().to_str().unwrap()));
            assert_eq!(
                fs::read_to_string(dir.path().join("src/lib.rs")).unwrap(),
                BAD
            );
            assert!(Fixture::create(dir.path(), mode).is_err());
            fs::write(dir.path().join("src/lib.rs"), GOOD).unwrap();
            assert!(fixture.verify().is_err());
        }
    }
}
