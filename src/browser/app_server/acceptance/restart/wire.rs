//! Explicit whole-app acceptance fault. Never selected by normal app startup.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Before,
    After,
}
impl Mode {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> anyhow::Result<Option<Self>> {
        let mut mode = None;
        for arg in args {
            if let Some(value) = arg.strip_prefix("--restart-wire-loss=") {
                anyhow::ensure!(mode.is_none(), "Repeated --restart-wire-loss");
                mode = Some(match value {
                    "before" => Self::Before,
                    "after" => Self::After,
                    _ => anyhow::bail!("--restart-wire-loss must be before or after"),
                });
            } else if arg == "--restart-wire-loss" {
                anyhow::bail!("Use --restart-wire-loss=before|after");
            }
        }
        Ok(mode)
    }
    pub(super) fn marker(self, root: &Path) -> anyhow::Result<Option<u32>> {
        let path = root.join("wire-marker.json");
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return Ok(None);
        };
        validate_marker(self, &value).map(Some)
    }
}

fn validate_marker(mode: Mode, value: &Value) -> anyhow::Result<u32> {
    let pid = value["nativePid"]
        .as_u64()
        .and_then(|id| u32::try_from(id).ok())
        .filter(|id| *id != 0 && *id != std::process::id())
        .context("Missing owned native relay child")?;
    anyhow::ensure!(
        value["starts"] == 2
            && value["acknowledged"] == (mode == Mode::After)
            && value["phase"]
                == if mode == Mode::After {
                    "completed-with-ack-lost"
                } else {
                    "request-dropped"
                },
        "Wire fault did not reach the selected native boundary: {value}"
    );
    Ok(pid)
}

pub(super) struct Fixture {
    executable: PathBuf,
    native: PathBuf,
    script: PathBuf,
}
impl Fixture {
    pub(super) fn create(root: &Path) -> anyhow::Result<Self> {
        let runtime = Runtime::discover(root).map_err(anyhow::Error::msg)?;
        // This opt-in diagnostic needs a checkout, but a distributed executable
        // must never retain the developer's absolute build directory.
        let source = std::env::var_os("SUPERVISOR_ACCEPTANCE_SOURCE_ROOT")
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)?;
        let tests = source.join("crates/central-agent-codex-runtime/tests");
        anyhow::ensure!(
            tests.join("native-wire-relay.rs").is_file()
                && tests.join("native-wire-loss.mjs").is_file(),
            "Run this acceptance check from the source checkout or set SUPERVISOR_ACCEPTANCE_SOURCE_ROOT"
        );
        let executable = root.join(if cfg!(windows) {
            "native-wire-relay.exe"
        } else {
            "native-wire-relay"
        });
        let mut compiler = std::process::Command::new("rustc");
        compiler
            .arg(tests.join("native-wire-relay.rs"))
            .args(["--edition", "2024", "-o"])
            .arg(&executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            compiler.creation_flags(0x08000000);
        }
        anyhow::ensure!(
            compiler.status()?.success(),
            "Could not compile the acceptance-only stdio relay"
        );
        Ok(Self {
            executable,
            native: runtime.executable().into(),
            script: tests.join("native-wire-loss.mjs"),
        })
    }
    pub(super) fn configure(&self, command: &mut std::process::Command, mode: Mode) {
        command
            .env(
                central_agent_codex_runtime::runtime::EXECUTABLE_OVERRIDE,
                &self.executable,
            )
            .env("CENTRAL_AGENT_WIRE_NATIVE", &self.native)
            .env("CENTRAL_AGENT_WIRE_SCRIPT", &self.script)
            .env("CENTRAL_AGENT_WIRE_MODE", mode.name());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_options_and_marker_require_exact_loss_evidence() {
        assert_eq!(
            Mode::parse(["--restart-wire-loss=before".into()]).unwrap(),
            Some(Mode::Before)
        );
        for args in [
            vec!["--restart-wire-loss"],
            vec!["--restart-wire-loss=unknown"],
            vec!["--restart-wire-loss=before", "--restart-wire-loss=after"],
        ] {
            assert!(Mode::parse(args.into_iter().map(str::to_owned)).is_err());
        }
        let marker = json!({"nativePid":u32::MAX,"starts":2,"acknowledged":true,"phase":"completed-with-ack-lost"});
        assert!(validate_marker(Mode::After, &marker).is_ok());
        assert!(validate_marker(Mode::Before, &marker).is_err());
        for (key, value) in [
            ("starts", json!(3)),
            ("acknowledged", json!(false)),
            ("phase", json!("native-failed")),
            ("nativePid", json!(0)),
        ] {
            let mut bad = marker.clone();
            bad[key] = value;
            assert!(validate_marker(Mode::After, &bad).is_err());
        }
    }
}
