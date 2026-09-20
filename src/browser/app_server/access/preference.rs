//! One small local preference, independent of native histories and credentials.
use super::*;
use std::io::Read;

pub(super) const FILE_NAME: &str = "app-server-permissions.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedAccess {
    version: u32,
    access: api::Access,
}

pub(super) fn load(data_dir: &Path) -> Result<api::Access, String> {
    let path = data_dir.join(FILE_NAME);
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // An older backup may contain broader access than the last choice.
            // Never restore permission consent from it automatically.
            return if backup_path_for(&path).exists() {
                Err("The saved permissions file is missing; its backup was preserved".into())
            } else {
                Ok(api::Access::ReadOnly)
            };
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 4096 {
        return Err("The saved permissions file is unexpectedly large".into());
    }
    let saved: SavedAccess = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if saved.version != 1 {
        return Err("Unsupported saved permissions version".into());
    }
    Ok(saved.access)
}

pub(super) fn save(data_dir: &Path, access: api::Access) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(&SavedAccess { version: 1, access })
        .map_err(|error| error.to_string())?;
    write_file_atomically::<SavedAccess>(&data_dir.join(FILE_NAME), &bytes)
        .map_err(|error| error.to_string())
}
