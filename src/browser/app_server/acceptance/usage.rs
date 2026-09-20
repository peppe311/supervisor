//! Compare real native usage events with the compiled DOM, not fabricated data.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const COUNTERS: [(&str, &str); 6] = [
    ("totalTokens", "Total"),
    ("inputTokens", "Input"),
    ("cachedInputTokens", "Cached input"),
    ("cacheWriteInputTokens", "Cache write input"),
    ("outputTokens", "Output"),
    ("reasoningOutputTokens", "Reasoning output"),
];

pub(super) fn sample(base: &str, owner: &str, graph: bool) -> String {
    format!(
        r#"(()=>{{const state=({base});const roots={roots};
      state.usage=roots.map(root=>{{const usage=root.querySelector('.native-usage');return {{
        owner:{owner_expr},present:!!usage,stale:usage?.dataset.stale,
        headline:usage?.querySelector('.usage-head strong')?.textContent,
        status:usage?.querySelector(':scope > p')?.textContent,
        meter:usage?.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')??null,
        rows:[...usage?.querySelectorAll('tbody tr')||[]].map(row=>[...row.children].map(cell=>cell.textContent))
      }};}});return state;}})()"#,
        roots = if graph {
            "[...document.querySelectorAll('.agent-console[data-node-key]')]"
        } else {
            "[document]"
        },
        owner_expr = if graph {
            "'graph:'+root.dataset.nodeKey".into()
        } else {
            json!(owner).to_string()
        }
    )
}

#[derive(Default)]
pub(super) struct Usage {
    reports: BTreeMap<String, Value>,
    turns: BTreeMap<String, BTreeSet<String>>,
    disconnected: bool,
}
impl Usage {
    pub(super) fn observe(&mut self, app: &BrowserApp, event: &BrowserEvent) {
        if let BrowserEvent::AppServer(Event::Transport {
            event: TransportEvent::Notification { method, params },
            ..
        }) = event
            && method == "thread/tokenUsage/updated"
            && let Some(owner) = params["threadId"]
                .as_str()
                .and_then(|id| app.app_server.conversations.owner_for_thread(id))
            && let Some(turn) = params["turnId"].as_str()
        {
            self.turns
                .entry(owner.into())
                .or_default()
                .insert(turn.into());
            self.reports.insert(owner.into(), params.clone());
        }
    }
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        state: &Value,
        graph: bool,
    ) -> anyhow::Result<bool> {
        if self.disconnected && app.app_server.view.connected {
            return Ok(false);
        }
        let book = &app.app_server.conversations;
        let Some(rows) = state["usage"].as_array() else {
            return Ok(false);
        };
        if rows.len() != if graph { 2 } else { 1 } {
            return Ok(false);
        }
        if self.reports.len() != rows.len() {
            return Ok(false);
        }
        let mut turns_seen = 0;
        for (owner, params) in &self.reports {
            let binding = book.binding(owner).context("Usage lost its native owner")?;
            let thread = book
                .mirror
                .thread(&binding.thread_id)
                .context("Usage lost native history")?;
            anyhow::ensure!(
                params["threadId"] == binding.thread_id
                    && params["turnId"].as_str() == thread.turns.last().map(|t| t.id.as_str()),
                "Usage is not scoped to the owner's most recent native turn"
            );
            anyhow::ensure!(
                thread.token_usage.as_ref() == Some(&params["tokenUsage"]),
                "Host counters differ from the actual native event"
            );
            anyhow::ensure!(
                thread.token_usage_current != self.disconnected,
                "Native usage freshness did not follow transport state"
            );
            let seen = &self.turns[owner];
            anyhow::ensure!(
                thread.turns.iter().all(|t| seen.contains(&t.id)),
                "A native turn never reported usage"
            );
            turns_seen += seen.len();
            let Some(row) = rows.iter().find(|r| r["owner"] == *owner) else {
                return Ok(false);
            };
            let report = &params["tokenUsage"];
            let expected: Vec<_> = COUNTERS
                .iter()
                .map(|(key, label)| {
                    json!([
                        label,
                        formatted(&report["last"][key]),
                        formatted(&report["total"][key])
                    ])
                })
                .collect();
            let (headline, meter) = headline(report);
            let expected_meter = meter.map(|m| m.to_string());
            if row["present"] != true
                || row["rows"] != json!(expected)
                || row["headline"] != headline
                || row["meter"] != json!(expected_meter)
                || row["stale"].as_str() != Some(if self.disconnected { "true" } else { "false" })
                || row["status"]
                    != if self.disconnected {
                        "Disconnected · last known report, not live data."
                    } else {
                        "Latest native report; not a per-token live count."
                    }
            {
                return Ok(false);
            }
        }
        anyhow::ensure!(
            turns_seen == if graph { 3 } else { 2 },
            "Unexpected number of owner-correlated native usage turns"
        );
        if !self.disconnected {
            println!(
                "Usage host: all six latest/total counters and capacity meter match real native events across {turns_seen} turns; no cross-owner mixing."
            );
            app.app_server
                .client
                .as_ref()
                .context("Missing owned usage client")?
                .shutdown();
            self.disconnected = true;
            return Ok(false);
        }
        println!(
            "Usage host: real disconnect retains exact counters but marks every displayed report stale, not live."
        );
        Ok(true)
    }
}

fn formatted(value: &Value) -> String {
    let Some(value) = value.as_u64() else {
        return "Not reported".into();
    };
    let digits = value.to_string();
    let mut output = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }
    output
}
fn headline(report: &Value) -> (String, Option<f64>) {
    let used = report["last"]["totalTokens"].as_u64();
    let capacity = report["modelContextWindow"].as_u64();
    let mut text = formatted(&report["last"]["totalTokens"]);
    if capacity.is_some() {
        text.push_str(&format!(" / {}", formatted(&report["modelContextWindow"])));
    } else {
        text.push_str(" tokens");
    }
    let basis = used
        .zip(capacity.filter(|n| *n > 0))
        .map(|(used, capacity)| u128::from(used) * 10000 / u128::from(capacity));
    if let Some(basis) = basis {
        let decimal = basis % 100;
        text.push_str(&format!(
            " · {}{}%",
            basis / 100,
            if decimal == 0 {
                String::new()
            } else {
                format!(".{decimal:02}")
            }
        ));
    }
    (text, basis.map(|n| n.min(10000) as f64 / 100.0))
}

#[cfg(test)]
#[test]
fn usage_oracle_keeps_latest_total_missing_zero_and_over_capacity_distinct() {
    assert_eq!(formatted(&json!(u64::MAX)), "18,446,744,073,709,551,615");
    assert_eq!(formatted(&Value::Null), "Not reported");
    assert_eq!(formatted(&json!(0)), "0");
    assert_eq!(
        headline(
            &json!({"last":{"totalTokens":25},"total":{"totalTokens":9000},"modelContextWindow":100})
        ),
        ("25 / 100 · 25%".into(), Some(25.0))
    );
    assert_eq!(
        headline(&json!({"last":{"totalTokens":125},"modelContextWindow":100})),
        ("125 / 100 · 125%".into(), Some(100.0))
    );
    assert_eq!(
        headline(&json!({"last":{"totalTokens":0},"modelContextWindow":0})),
        ("0 / 0".into(), None)
    );
    assert!(sample("({})", "chat:fixture", false).contains("\"chat:fixture\""));
    assert!(sample("({})", "ignored", true).contains("'graph:'+root.dataset.nodeKey"));
}
