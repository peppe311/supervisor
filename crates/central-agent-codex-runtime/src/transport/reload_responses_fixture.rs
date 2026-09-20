// Test-only loopback Responses fixture. Native App Server still owns turns,
// persistence and config reload; no remote model or auth is involved.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) struct Fixture {
    pub address: SocketAddr,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl Fixture {
    pub fn start() -> std::io::Result<Self> {
        Self::with_output(ready_output)
    }
    pub(super) fn with_output(
        output: fn(&Value, usize) -> Result<Value, String>,
    ) -> std::io::Result<Self> {
        Self::start_with(output, false)
    }
    pub(super) fn with_summary_stream() -> std::io::Result<Self> {
        Self::start_with(ready_output, true)
    }
    fn start_with(
        output: fn(&Value, usize) -> Result<Value, String>,
        summaries: bool,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let signal = stop.clone();
        let captured = requests.clone();
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        if !peer.ip().is_loopback() {
                            return Err("Nonlocal fixture request".into());
                        }
                        serve(stream, &captured, output, summaries)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            Ok(())
        });
        Ok(Self {
            address,
            requests,
            stop,
            worker: Some(worker),
        })
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| "Responses fixture panicked".to_owned())?
            })
            .unwrap_or(Ok(()))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn serve(
    mut stream: TcpStream,
    requests: &Mutex<Vec<Value>>,
    output: fn(&Value, usize) -> Result<Value, String>,
    summaries: bool,
) -> Result<(), String> {
    // Windows accepted sockets inherit the listener's nonblocking mode.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let header_end = loop {
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("Fixture request ended before headers".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() > 65536 {
            return Err("Fixture headers too large".into());
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end]).map_err(|e| e.to_string())?;
    if !headers.starts_with("POST /v1/responses HTTP/1.1\r\n")
        || headers.to_ascii_lowercase().contains("\r\nauthorization:")
    {
        return Err("Fixture accepts only unauthenticated local Responses requests".into());
    }
    let length = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        })
        .ok_or("Missing fixture content length")?;
    if length > 4 * 1024 * 1024 {
        return Err("Fixture request body too large".into());
    }
    while bytes.len() < header_end + length {
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("Incomplete fixture body".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let body: Value = serde_json::from_slice(&bytes[header_end..header_end + length])
        .map_err(|_| "Invalid fixture JSON")?;
    let mut capture = requests.lock().map_err(|_| "Fixture lock poisoned")?;
    let index = capture.len() + 1;
    // Store only settings under test, never input, tools, headers or instructions.
    capture.push(json!({"model":body["model"],"reasoning":body["reasoning"],"service_tier":body["service_tier"]}));
    drop(capture);
    let item = output(&body, index)?;
    let mut events =
        vec![json!({"type":"response.created","response":{"id":format!("response-{index}")}})];
    let mut items = Vec::new();
    if summaries {
        let reasoning_id = format!("reasoning-{index}");
        events.push(json!({"type":"response.output_item.added","output_index":0,"item":{"type":"reasoning","id":reasoning_id,"summary":[]}}));
        let parts = [
            vec!["Checking ", "the fixture."],
            vec!["Ready ", "to answer."],
        ];
        let mut summary = Vec::new();
        for (part_index, deltas) in parts.iter().enumerate() {
            events.push(json!({"type":"response.reasoning_summary_part.added","item_id":reasoning_id,"output_index":0,"summary_index":part_index,"part":{"type":"summary_text","text":""}}));
            for delta in deltas {
                events.push(json!({"type":"response.reasoning_summary_text.delta","item_id":reasoning_id,"output_index":0,"summary_index":part_index,"delta":delta}));
            }
            let text = deltas.concat();
            let part = json!({"type":"summary_text","text":text});
            events.push(json!({"type":"response.reasoning_summary_text.done","item_id":reasoning_id,"output_index":0,"summary_index":part_index,"text":text}));
            events.push(json!({"type":"response.reasoning_summary_part.done","item_id":reasoning_id,"output_index":0,"summary_index":part_index,"part":part}));
            summary.push(part);
        }
        let reasoning = json!({"type":"reasoning","id":reasoning_id,"summary":summary});
        events.push(json!({"type":"response.output_item.done","output_index":0,"item":reasoning}));
        items.push(reasoning);
        let progress_id = format!("progress-{index}");
        events.push(json!({"type":"response.output_item.added","output_index":1,"item":{"type":"message","id":progress_id,"role":"assistant","content":[],"phase":"commentary","status":"in_progress"}}));
        events.push(json!({"type":"response.content_part.added","output_index":1,"item_id":progress_id,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}));
        for delta in ["Fixture ", "checked."] {
            events.push(json!({"type":"response.output_text.delta","output_index":1,"item_id":progress_id,"content_index":0,"delta":delta}));
        }
        let progress = json!({"type":"message","id":progress_id,"role":"assistant","phase":"commentary","status":"completed","content":[{"type":"output_text","text":"Fixture checked.","annotations":[]}]});
        events.push(json!({"type":"response.output_item.done","output_index":1,"item":progress}));
        items.push(progress);
    }
    events.push(json!({"type":"response.output_item.done","output_index":items.len(),"item":item}));
    items.push(item);
    let response = json!({"id":format!("response-{index}"),"object":"response","created_at":1,"status":"completed","model":body["model"],"output":items,"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}});
    events.push(json!({"type":"response.completed","response":response}));
    for (sequence, event) in events.iter_mut().enumerate() {
        event["sequence_number"] = json!(sequence);
    }
    let payload = events
        .iter()
        .map(|event| {
            format!(
                "event: {}\ndata: {}\n\n",
                event["type"].as_str().unwrap(),
                event
            )
        })
        .collect::<String>();
    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",payload.len(),payload).map_err(|e|e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

pub(super) fn ready_output(_: &Value, index: usize) -> Result<Value, String> {
    Ok(
        json!({"id":format!("message-{index}"),"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"READY","annotations":[]}],"phase":"final_answer"}),
    )
}

pub(super) fn mcp_output(body: &Value, index: usize) -> std::result::Result<Value, String> {
    let input = body["input"].as_array().ok_or("Missing fixture input")?;
    let last = input
        .iter()
        .rposition(|i| i["role"] == "user")
        .ok_or("Missing user input")?;
    if input[last + 1..]
        .iter()
        .any(|i| i["type"] == "function_call_output" || i["type"] == "custom_tool_call_output")
    {
        return ready_output(body, index);
    }
    let text = input[last].to_string();
    let mode = if text.contains("MCP_FIXTURE_FORM") {
        "form"
    } else if text.contains("MCP_FIXTURE_URL") {
        "url"
    } else {
        return Err("Missing fixture marker".into());
    };
    let tools = body["tools"]
        .as_array()
        .or_else(|| {
            input
                .iter()
                .find(|i| i["type"] == "additional_tools")
                .and_then(|i| i["tools"].as_array())
        })
        .ok_or("Missing native tool declarations")?;
    let name = "mcp__elicitation_fixture__fixture_question";
    if tools.iter().any(|t| {
        t["type"] == "namespace"
            && t["name"] == "functions"
            && t["tools"].as_array().is_some_and(|ts| {
                ts.iter()
                    .any(|t| t["type"] == "custom" && t["name"] == "exec")
            })
    }) {
        let code = format!(
            "const t = ALL_TOOLS.find(t => t.name.includes('elicitation_fixture') && t.name.endsWith('fixture_question')); if (!t) throw new Error('Missing owned MCP fixture'); text(await tools[t.name]({{mode:{}}}));",
            json!(mode)
        );
        return Ok(
            json!({"id":format!("function-{index}"),"type":"custom_tool_call","call_id":format!("mcp-call-{index}"),"name":"exec","namespace":"functions","input":code,"status":"completed"}),
        );
    }
    if !tools.iter().any(|tool| tool["name"] == name) {
        return Err(format!(
            "Fixture MCP tool not exposed: {}",
            json!(
                tools
                    .iter()
                    .map(|t| json!({"type":t["type"],"name":t["name"],"tools":t["tools"].as_array().map(|ts|ts.iter().map(|v|json!({"type":v["type"],"name":v["name"]})).collect::<Vec<_>>())}))
                    .collect::<Vec<_>>()
            )
        ));
    }
    Ok(
        json!({"id":format!("function-{index}"),"type":"function_call","call_id":format!("mcp-call-{index}"),"name":name,"arguments":json!({"mode":mode}).to_string(),"status":"completed"}),
    )
}
