//! Single bounded background writer; never blocks a prompt on diagnostic I/O.
use super::*;
use std::sync::{Arc, Mutex, mpsc};

enum Work {
    Entry(Box<Entry>),
    #[cfg(test)]
    Flush(mpsc::Sender<()>),
}

pub(super) struct Writer {
    sender: Option<mpsc::SyncSender<Work>>,
    worker: Option<std::thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Writer {
    pub(super) fn new(directory: PathBuf, mut count: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(256);
        let error = Arc::new(Mutex::new(None));
        let failure = error.clone();
        let worker = std::thread::spawn(move || {
            for work in receiver {
                match work {
                    Work::Entry(entry) => {
                        if failure.lock().unwrap().is_some() {
                            continue;
                        }
                        if let Err(message) = append(&directory, &mut count, &entry) {
                            *failure.lock().unwrap() = Some(message);
                        }
                    }
                    #[cfg(test)]
                    Work::Flush(reply) => {
                        let _ = reply.send(());
                    }
                }
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
            error,
        }
    }

    pub(super) fn append(&self, entry: Entry) -> Result<(), String> {
        self.sender
            .as_ref()
            .unwrap()
            .try_send(Work::Entry(Box::new(entry)))
            .map_err(|_| {
                "Event log writer is unavailable or full; new events remain in memory".into()
            })
    }
    pub(super) fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }

    #[cfg(test)]
    pub(super) fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Work::Flush(sender))
            .unwrap();
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn append(directory: &Path, count: &mut usize, entry: &Entry) -> Result<(), String> {
    let current = directory.join(CURRENT);
    if *count >= SEGMENT {
        let previous = directory.join(PREVIOUS);
        if previous.exists() {
            fs::remove_file(&previous)
                .map_err(|_| "Event log could not rotate its previous segment")?;
        }
        fs::rename(&current, previous)
            .map_err(|_| "Event log could not rotate its current segment")?;
        *count = 0;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(current)
        .map_err(|_| "Event log could not be saved")?;
    serde_json::to_writer(&mut file, entry).map_err(|_| "Event log could not be saved")?;
    file.write_all(b"\n")
        .map_err(|_| "Event log could not be saved")?;
    *count += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Entry {
        Entry {
            sequence: 1,
            received_at_ms: 1,
            connection_epoch: 1,
            direction: "dispatch".into(),
            method: "turn/start".into(),
            owner: Some("chat:a".into()),
            thread_id: None,
            turn_id: None,
            item_id: None,
            request_id: None,
            state: None,
            summary_index: None,
            delta_bytes: None,
        }
    }
    #[test]
    fn a_full_diagnostic_queue_returns_immediately_instead_of_waiting_for_disk() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let writer = Writer {
            sender: Some(sender),
            worker: None,
            error: Arc::new(Mutex::new(None)),
        };
        writer.append(entry()).unwrap();
        assert!(writer.append(entry()).is_err());
    }
    #[test]
    fn asynchronous_disk_failure_is_reported_without_failing_delivery() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("not-a-directory");
        fs::write(&path, b"fixture").unwrap();
        let writer = Writer::new(path, 0);
        writer.append(entry()).unwrap();
        writer.flush();
        assert!(writer.error().is_some());
    }
}
