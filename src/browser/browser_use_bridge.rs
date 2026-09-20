use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc as std_mpsc,
    },
    thread,
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::{Semaphore, mpsc, oneshot},
};
use winit::event_loop::EventLoopProxy;

use super::BrowserEvent;

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const START_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_QUEUE_CAPACITY: usize = 1_024;
const MAX_IN_FLIGHT_REQUESTS: usize = 64;

static ACTIVE_PIPE: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

pub(crate) struct Request {
    pub(super) method: String,
    pub(super) params: Value,
    pub(super) reply: oneshot::Sender<Result<Value, String>>,
}

#[derive(Clone)]
pub(super) struct Handle {
    inner: Arc<Inner>,
    owner: Arc<()>,
}

struct Inner {
    pipe_path: String,
    running: AtomicBool,
    hub: Arc<Hub>,
}

#[derive(Default)]
struct Hub {
    next_id: AtomicU64,
    peers: Mutex<HashMap<u64, mpsc::Sender<Value>>>,
}

impl Hub {
    fn insert(&self, sender: mpsc::Sender<Value>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, sender);
        id
    }

    fn remove(&self, id: u64) {
        self.peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
    }

    fn broadcast(&self, message: Value) {
        self.peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|_, sender| match sender.try_send(message.clone()) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            });
    }
}

impl Handle {
    pub(super) fn notify_cdp_event(
        &self,
        tab_id: u64,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) {
        let mut source = json!({ "tabId": tab_id });
        if let Some(session_id) = session_id.filter(|value| !value.is_empty()) {
            source["sessionId"] = Value::String(session_id.to_owned());
        }
        self.inner.hub.broadcast(json!({
            "jsonrpc": "2.0",
            "method": "onCDPEvent",
            "params": {
                "source": source,
                "method": method,
                "params": params,
            },
        }));
    }

    pub(super) fn notify_cdp_detach(&self, tab_id: u64) {
        self.inner.hub.broadcast(json!({
            "jsonrpc": "2.0",
            "method": "onCDPDetach",
            "params": { "tabId": tab_id },
        }));
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.owner) != 1 {
            return;
        }
        self.inner.running.store(false, Ordering::Release);
        let mut active = ACTIVE_PIPE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.as_deref() == Some(self.inner.pipe_path.as_str()) {
            *active = None;
        }
        // Connecting wakes a listener blocked in `connect` so it can observe
        // shutdown. Failure is harmless when no listener is pending.
        let _ = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.inner.pipe_path);
    }
}

pub(super) fn pipe_path_if_running() -> Option<String> {
    ACTIVE_PIPE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(super) fn start(proxy: EventLoopProxy<BrowserEvent>) -> Result<Handle, String> {
    let pipe_path = format!(
        r"\\.\pipe\codex-browser-use-supervisor-{}",
        std::process::id()
    );
    let inner = Arc::new(Inner {
        pipe_path: pipe_path.clone(),
        running: AtomicBool::new(true),
        hub: Arc::new(Hub::default()),
    });
    let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
    let server_inner = Arc::clone(&inner);
    thread::Builder::new()
        .name("supervisor-browser-use-bridge".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(error.to_string()));
                    return;
                }
            };
            if let Err(error) = runtime.block_on(serve(server_inner, proxy, ready_tx)) {
                tracing::warn!(%error, "Supervisor Browser Use bridge stopped");
            }
        })
        .map_err(|error| error.to_string())?;

    ready_rx
        .recv_timeout(START_TIMEOUT)
        .map_err(|_| "Supervisor Browser Use bridge did not start in time".to_owned())??;
    *ACTIVE_PIPE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(pipe_path);
    Ok(Handle {
        inner,
        owner: Arc::new(()),
    })
}

async fn serve(
    inner: Arc<Inner>,
    proxy: EventLoopProxy<BrowserEvent>,
    ready: std_mpsc::SyncSender<Result<(), String>>,
) -> io::Result<()> {
    let mut first_instance = true;
    let mut ready = Some(ready);
    while inner.running.load(Ordering::Acquire) {
        let server = match ServerOptions::new()
            .first_pipe_instance(first_instance)
            .create(&inner.pipe_path)
        {
            Ok(server) => server,
            Err(error) => {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(error.to_string()));
                }
                return Err(error);
            }
        };
        first_instance = false;
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(()));
        }
        server.connect().await?;
        if !inner.running.load(Ordering::Acquire) {
            break;
        }
        let connection_proxy = proxy.clone();
        let hub = Arc::clone(&inner.hub);
        tokio::spawn(async move {
            if let Err(error) = serve_connection(server, connection_proxy, Arc::clone(&hub)).await {
                tracing::debug!(%error, "Browser Use bridge client disconnected");
            }
        });
    }
    Ok(())
}

async fn serve_connection(
    stream: NamedPipeServer,
    proxy: EventLoopProxy<BrowserEvent>,
    hub: Arc<Hub>,
) -> io::Result<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (event_tx, mut event_rx) = mpsc::channel::<Value>(EVENT_QUEUE_CAPACITY);
    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Value>();
    let peer_id = hub.insert(event_tx.clone());
    let writer_task = tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                response = response_rx.recv() => response,
                event = event_rx.recv() => event,
            };
            let Some(message) = message else {
                break;
            };
            write_frame(&mut writer, &message).await?;
        }
        Ok::<(), io::Error>(())
    });
    let request_slots = Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS));

    let read_result = async {
        while let Some(message) = read_frame(&mut reader).await? {
            let Some(method) = message.get("method").and_then(Value::as_str) else {
                continue;
            };
            let Some(id) = message.get("id").cloned() else {
                // The official service currently uses this notification only
                // for optional WebMCP telemetry. The WebView backend has no
                // state to update for it.
                continue;
            };
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let request_proxy = proxy.clone();
            let response_tx = response_tx.clone();
            let method = method.to_owned();
            let Ok(permit) = Arc::clone(&request_slots).acquire_owned().await else {
                break;
            };
            tokio::spawn(async move {
                let _permit = permit;
                let (reply_tx, reply_rx) = oneshot::channel();
                let request = Request {
                    method,
                    params,
                    reply: reply_tx,
                };
                let result = if request_proxy
                    .send_event(BrowserEvent::BrowserUseBridge(request))
                    .is_err()
                {
                    Err("Supervisor's browser is no longer available".to_owned())
                } else {
                    match tokio::time::timeout(REQUEST_TIMEOUT, reply_rx).await {
                        Ok(Ok(result)) => result,
                        Ok(Err(_)) => Err("Supervisor's browser request was cancelled".to_owned()),
                        Err(_) => Err("Supervisor's browser request timed out".to_owned()),
                    }
                };
                let response = match result {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err(message) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -1, "message": message },
                    }),
                };
                let _ = response_tx.send(response);
            });
        }
        Ok::<(), io::Error>(())
    }
    .await;

    hub.remove(peer_id);
    drop(event_tx);
    drop(response_tx);
    let _ = writer_task.await;
    read_result
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut header = [0_u8; 4];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = u32::from_ne_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Browser Use frame length",
        ));
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser Use response exceeds the frame limit",
        ));
    }
    writer
        .write_all(&(payload.len() as u32).to_ne_bytes())
        .await?;
    writer.write_all(&payload).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn framed_json_uses_native_u32_length_and_round_trips() {
        let expected = json!({"jsonrpc":"2.0","id":7,"result":{"ok":true}});
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &expected).await.unwrap();
        assert_eq!(
            u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as usize,
            bytes.len() - 4
        );
        let mut input = bytes.as_slice();
        assert_eq!(read_frame(&mut input).await.unwrap(), Some(expected));
    }

    #[tokio::test]
    async fn framed_json_rejects_oversized_messages_before_allocation() {
        let bytes = ((MAX_FRAME_BYTES as u32) + 1).to_ne_bytes();
        let mut input = bytes.as_slice();
        let error = read_frame(&mut input).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
