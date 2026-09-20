//! One-time removal of our retired skill. Never changes the vendor package,
//! user-owned skills, shared skill preferences or unrelated files.
use super::*;

pub(super) const NAME: &str = "supervisor-computer-use";
const OWNER: &str = "<!-- Supervisor-managed Computer Use companion -->";

pub(super) fn remove_owned(home: &Path) -> Result<(), String> {
    let home = home.canonicalize().map_err(|e| e.to_string())?;
    let mut directory = home.clone();
    for part in ["skills", NAME] {
        directory.push(part);
        match fs::symlink_metadata(&directory) {
            Ok(meta) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    meta.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = false;
                if linked
                    || meta.file_type().is_symlink()
                    || !meta.is_dir()
                    || !directory
                        .canonicalize()
                        .map_err(|e| e.to_string())?
                        .starts_with(&home)
                {
                    return Err("The retired Computer Use skill cannot be removed through a filesystem link".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
    }
    let path = directory.join("SKILL.md");
    if !regular_file(&path)? || fs::metadata(&path).map_err(|e| e.to_string())?.len() > 32 * 1024 {
        return Ok(());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !content.lines().any(|line| line == OWNER) {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    // Remove an empty owned directory only. Never recurse into other content.
    let _ = fs::remove_dir(&directory);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_managed_instructions_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("skills").join(NAME).join("SKILL.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("---\nname: {NAME}\n---\n{OWNER}\n")).unwrap();
        let notes = path.with_file_name("user-notes.md");
        fs::write(&notes, "preserve").unwrap();
        let config = temp.path().join("config.toml");
        fs::write(&config, "model = 'keep'\n").unwrap();
        remove_owned(temp.path()).unwrap();
        remove_owned(temp.path()).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(notes).unwrap(), "preserve");
        assert_eq!(fs::read_to_string(config).unwrap(), "model = 'keep'\n");
    }

    #[test]
    fn preserves_user_skill_and_does_not_create_missing_directories() {
        let temp = tempfile::tempdir().unwrap();
        remove_owned(temp.path()).unwrap();
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let path = temp.path().join("skills").join(NAME).join("SKILL.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let content = format!("User notes mention {OWNER} inline.");
        fs::write(&path, &content).unwrap();
        remove_owned(temp.path()).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }

    #[test]
    fn rejects_non_directory_skill_location() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("skills"), "keep").unwrap();
        assert!(remove_owned(temp.path()).is_err());
        assert_eq!(
            fs::read_to_string(temp.path().join("skills")).unwrap(),
            "keep"
        );
    }
}
