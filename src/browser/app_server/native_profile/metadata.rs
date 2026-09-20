//! Titles and goals live outside rollouts. Read them through the public API;
//! never copy the shared databases or write into the original native profile.
use super::hydration::{call, connect};
use super::*;
use central_agent_codex_runtime::{api, goals, transport::Client};
use std::sync::mpsc;

pub(super) fn capture(runtime: &Runtime, report: &mut Report) -> Result<(), String> {
    if report.metadata_captured {
        return Ok(());
    }
    if !report.copied.is_empty() {
        let source = report
            .source
            .as_deref()
            .ok_or("The original profile is unavailable")?;
        let (client, events) = connect(&runtime.clone().with_existing_home(source)?)?;
        let outcome: Result<(), String> = (|| {
            let mut captured = BTreeMap::new();
            for id in report.copied.keys() {
                let thread = call(&client, &events, api::read_thread_metadata(id))?;
                let name = thread["thread"]["name"].as_str().map(str::to_owned);
                if name.as_ref().is_some_and(|name| name.len() > 4096) {
                    return Err("A native title exceeds its transfer limit".into());
                }
                let response = call(&client, &events, goals::get(id))?;
                let goal: Option<goals::Goal> = serde_json::from_value(response["goal"].clone())
                    .map_err(|error| error.to_string())?;
                if goal
                    .as_ref()
                    .is_some_and(|goal| goal.thread_id != *id || goal.objective.len() > 65536)
                {
                    return Err("Invalid native goal metadata".into());
                }
                captured.insert(id.clone(), Metadata { name, goal });
            }
            report.metadata = captured;
            report.metadata_captured = true;
            Ok(())
        })();
        client.shutdown();
        outcome?;
    } else {
        report.metadata_captured = true;
    }
    Ok(())
}

pub(super) fn import(
    client: &Client,
    events: &mpsc::Receiver<central_agent_codex_runtime::transport::Event>,
    id: &str,
    metadata: Option<&Metadata>,
) -> Result<(), String> {
    let Some(metadata) = metadata else {
        return Ok(());
    };
    if let Some(name) = &metadata.name {
        call(client, events, api::rename_thread(id, name))?;
    }
    if let Some(goal) = &metadata.goal {
        let Some((status, budget)) = goal_settings(goal) else {
            return Ok(());
        };
        let current = call(client, events, goals::get(id))?;
        if !current["goal"].is_null() {
            let current: goals::Goal = serde_json::from_value(current["goal"].clone())
                .map_err(|error| error.to_string())?;
            if current.objective != goal.objective
                || current.status != status
                || current.token_budget != budget
            {
                return Err("A goal in the separate profile changed during transfer".into());
            }
        } else {
            call(
                client,
                events,
                api::Call {
                    method: "thread/goal/set",
                    params: serde_json::json!({
                        "threadId":id,"objective":goal.objective,"status":status,"tokenBudget":budget
                    }),
                },
            )?;
        }
    }
    Ok(())
}

// The public write API cannot restore past counters. Keep the complete original
// goal in the migration record; use only its remaining allowance in this profile
// and never restart unfinished work. Completed goals remain completed.
fn goal_settings(goal: &goals::Goal) -> Option<(goals::Status, Option<u64>)> {
    if goal.status != goals::Status::Complete
        && goal
            .token_budget
            .is_some_and(|limit| goal.tokens_used >= limit)
    {
        return None;
    }
    let status = if goal.status == goals::Status::Complete {
        goals::Status::Complete
    } else {
        goals::Status::Paused
    };
    let budget = goal
        .token_budget
        .map(|limit| limit.saturating_sub(goal.tokens_used))
        .filter(|limit| *limit > 0);
    Some((status, budget))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_goals_never_restart_work_or_replenish_spent_budget() {
        let mut goal = goals::Goal {
            thread_id: "native".into(),
            objective: "Verify".into(),
            status: goals::Status::Active,
            token_budget: Some(100),
            tokens_used: 60,
            time_used_seconds: 10,
            created_at: 1,
            updated_at: 2,
        };
        assert_eq!(
            goal_settings(&goal),
            Some((goals::Status::Paused, Some(40)))
        );
        goal.tokens_used = 100;
        assert_eq!(goal_settings(&goal), None);
        goal.status = goals::Status::Complete;
        goal.token_budget = None;
        assert_eq!(goal_settings(&goal), Some((goals::Status::Complete, None)));
    }
}
