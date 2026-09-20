//! Remember connection intent, never credentials or runtime/session authority.
//! Codex owns cached ChatGPT authentication and token refresh through account/read.
use super::*;
use std::io::Read;

const FILE_NAME: &str = "app-server-connection.json";
const ERROR_KEY: &str = "connection-preference";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedConnection {
    version: u32,
    reconnect: bool,
}

#[derive(Default)]
pub(super) struct Connection {
    saved: Option<bool>,
    startup_pending: bool,
    history_startup_pending: bool,
    // Logout closes this gate before its RPC. Late account replies cannot undo it.
    remember_allowed: bool,
}

impl Connection {
    fn load(data_dir: &Path, previously_used: bool) -> Result<Self, String> {
        let path = data_dir.join(FILE_NAME);
        let saved = match fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(4097)
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                if bytes.len() > 4096 {
                    return Err("Saved connection preference is unexpectedly large".into());
                }
                let saved: SavedConnection =
                    serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
                if saved.version != 1 {
                    return Err("Unsupported saved connection preference version".into());
                }
                Some(saved.reconnect)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A backup may predate an explicit logout. Never resurrect it,
                // including when the primary disappeared after a failed write.
                if backup_path_for(&path).exists() {
                    return Err("Saved connection preference is missing; its backup was not restored automatically".into());
                }
                None
            }
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self {
            saved,
            // Upgrade older profiles that already used native Codex, but don't
            // start a runtime on a new profile or override a recorded sign-out.
            startup_pending: saved.unwrap_or(previously_used),
            history_startup_pending: false,
            remember_allowed: false,
        })
    }

    pub(super) fn begin_connect(&mut self) {
        self.startup_pending = false;
        self.history_startup_pending = false;
        self.remember_allowed = true;
    }

    pub(super) fn begin_login(&mut self) {
        self.remember_allowed = true;
    }

    fn save(&mut self, data_dir: &Path, reconnect: bool) -> Result<(), String> {
        if self.saved == Some(reconnect) {
            return Ok(());
        }
        let saved = SavedConnection {
            version: 1,
            reconnect,
        };
        let bytes = serde_json::to_vec_pretty(&saved).map_err(|error| error.to_string())?;
        write_file_atomically::<SavedConnection>(&data_dir.join(FILE_NAME), &bytes)
            .map_err(|error| error.to_string())?;
        self.saved = Some(reconnect);
        Ok(())
    }
}

impl State {
    pub(super) fn load_connection_preference(&mut self, data_dir: &Path) {
        let previously_used = !self.conversations.saved().bindings.is_empty()
            || !self.profile.selection.model.is_empty();
        match Connection::load(data_dir, previously_used) {
            Ok(mut connection) => {
                // Local history does not require a signed-in account. A recorded
                // sign-out still prevents OAuth and does not restore work.
                connection.history_startup_pending = self
                    .conversations
                    .saved()
                    .bindings
                    .values()
                    .any(|binding| !binding.deleted);
                self.connection = connection;
            }
            Err(error) => {
                self.view.errors.insert(ERROR_KEY, format!("Automatic Codex connection is paused: {error}. The saved file was preserved. Connect in AI accounts to retry."));
            }
        }
    }

    pub(in crate::browser) fn take_startup_connection(&mut self) -> bool {
        // Consumed even if the panel becomes ready more than once. Failures
        // offer the existing explicit Connect action, never a retry loop.
        let account = std::mem::take(&mut self.connection.startup_pending);
        let history = std::mem::take(&mut self.connection.history_startup_pending);
        (account || history)
            && self.binding_lock.is_some()
            && self.binding_store_error.is_none()
            && !self.view.connected
            && !self.view.connecting
    }

    pub(super) fn accept_account(&mut self, data_dir: &Path, value: &Value) -> Result<(), String> {
        let account = value
            .get("account")
            .ok_or("Account response is incomplete")?;
        let account = if account.is_null() {
            None
        } else {
            Some(parse_account(account)?)
        };
        let authenticated = account.as_ref().is_some_and(|account| account.supported);
        self.view.account = account;
        if self.connection.remember_allowed {
            let result = self.save_connection_preference(data_dir, authenticated);
            self.connection_preference_result(result);
        }
        // Persistence failure must be visible, but isn't an authentication failure.
        Ok(())
    }

    pub(super) fn forget_connection_for_logout(&mut self, data_dir: &Path) -> bool {
        self.connection.startup_pending = false;
        self.connection.history_startup_pending = false;
        self.connection.remember_allowed = false;
        let result = self.save_connection_preference(data_dir, false);
        let saved = result.is_ok();
        self.connection_preference_result(result);
        saved
    }

    fn save_connection_preference(
        &mut self,
        data_dir: &Path,
        reconnect: bool,
    ) -> Result<(), String> {
        if self.binding_lock.is_none() {
            return Err(
                "Another application instance owns this profile, or its storage cannot be locked"
                    .into(),
            );
        }
        self.connection.save(data_dir, reconnect)
    }

    fn connection_preference_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.view.errors.remove(ERROR_KEY);
            }
            Err(error) => {
                self.view.errors.insert(ERROR_KEY, format!("The Codex connection preference could not be saved: {error}. Check local storage and try again; sign-out cannot proceed until automatic connection can be disabled."));
            }
        }
    }
}

#[cfg(test)]
mod tests;
