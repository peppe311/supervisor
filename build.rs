fn main() {
    println!("cargo:rerun-if-changed=assets/windows.manifest");
    println!("cargo:rerun-if-changed=assets/supervisor.rc");
    println!("cargo:rerun-if-changed=assets/supervisor-app-icon.ico");
    println!("cargo:rerun-if-changed=assets/supervisor-app-icon.png");
    println!("cargo:rerun-if-changed=assets/supervisor-window-light.ico");
    println!("cargo:rerun-if-changed=assets/supervisor-window-dark.ico");
    println!("cargo:rerun-if-changed=assets/fonts/inter/Inter-Variable.woff2");
    println!("cargo:rerun-if-changed=ui/dist/central-agent-ui.css");
    println!("cargo:rerun-if-changed=ui/dist/central-agent-ui.js");
    #[cfg(windows)]
    {
        let manifest = std::path::Path::new("assets/windows.manifest")
            .canonicalize()
            .expect("Windows manifest must exist");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        let resource =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("supervisor.res");
        use std::os::windows::process::CommandExt;
        let mut compiler = std::process::Command::new(resource_compiler());
        compiler
            .arg("/nologo")
            .arg("/fo")
            .arg(&resource)
            .arg("assets/supervisor.rc")
            .creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let status = compiler
            .status()
            .expect("Windows SDK rc.exe is required to embed the Supervisor icon");
        assert!(
            status.success(),
            "Compiling Supervisor Windows resources failed"
        );
        println!("cargo:rustc-link-arg={}", resource.display());
    }
}

#[cfg(windows)]
fn resource_compiler() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("WindowsSdkVerBinPath") {
        let compiler = std::path::PathBuf::from(path).join("x64/rc.exe");
        if compiler.is_file() {
            return compiler;
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let compiler = directory.join("rc.exe");
            if compiler.is_file() {
                return compiler;
            }
        }
    }
    let kits = std::env::var_os("ProgramFiles(x86)")
        .map(std::path::PathBuf::from)
        .expect("Windows SDK location is unavailable")
        .join("Windows Kits/10/bin");
    let mut versions: Vec<_> = std::fs::read_dir(kits)
        .expect("Install the Windows SDK resource compiler")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.is_file())
        .collect();
    versions.sort();
    versions.pop().expect("Windows SDK rc.exe was not found")
}
