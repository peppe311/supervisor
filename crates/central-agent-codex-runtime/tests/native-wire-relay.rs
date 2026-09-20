//! Standalone acceptance launcher, compiled only inside a parent-owned temp root.
//! No protocol engine: version comes from Codex; the Node fixture relays stdio.
use std::{env, fs, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?.canonicalize()?;
    let root = executable.parent().ok_or("Missing relay root")?;
    let expected =
        PathBuf::from(env::var_os("CENTRAL_AGENT_RESTART_TEST_ROOT").ok_or("Missing owned root")?)
            .canonicalize()?;
    let token = env::var("CENTRAL_AGENT_RESTART_TEST_TOKEN")?;
    if root != expected
        || fs::read_to_string(root.join("owner-token"))? != token
        || !root.starts_with(env::temp_dir().canonicalize()?)
        || !env::current_dir()?.canonicalize()?.starts_with(root)
    {
        return Err("Relay is outside its owned acceptance profile".into());
    }
    let native =
        PathBuf::from(env::var_os("CENTRAL_AGENT_WIRE_NATIVE").ok_or("Missing native executable")?);
    if !native.is_absolute() || !native.is_file() || native.canonicalize()? == executable {
        return Err("Invalid official executable".into());
    }
    let args: Vec<_> = env::args().skip(1).collect();
    let mut command = if args == ["--version"] {
        let mut command = Command::new(native);
        command.arg("--version");
        command
    } else if args == ["app-server", "--listen", "stdio://"] {
        let script =
            PathBuf::from(env::var_os("CENTRAL_AGENT_WIRE_SCRIPT").ok_or("Missing test relay")?);
        let mode = env::var("CENTRAL_AGENT_WIRE_MODE")?;
        if !script.is_absolute()
            || !script.is_file()
            || !matches!(mode.as_str(), "before" | "after")
        {
            return Err("Invalid test relay arguments".into());
        }
        let mut command = Command::new("node");
        command
            .arg(script)
            .arg(native)
            .arg(root.join("wire-marker.json"))
            .arg(mode);
        command
    } else {
        return Err("Unexpected test relay command".into());
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let status = command.status()?;
    std::process::exit(status.code().unwrap_or(1));
}
