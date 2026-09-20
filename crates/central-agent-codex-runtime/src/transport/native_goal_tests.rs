//! Actual App Server goal state, isolated from accounts and personal histories.
//! App Server owns activation and continuation; the client sends no turn/start.
use super::reload_responses_fixture;
use crate::{
    api,
    goals::{self, Edit, Goal, Status, UserStatus},
    runtime::Runtime,
    transport::{Client, Event},
};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn connect(native: &Path, work: &Path) -> Result<(Client, mpsc::Receiver<Event>)> {
    let mut command = Runtime::discover(work)?.command();
    command
        .env("CODEX_HOME", native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "native-goal-probe", move |event| {
        let _ = tx.send(event);
    })?;
    loop {
        match rx.recv_timeout(Duration::from_secs(30))? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
            _ => {}
        }
    }
    if !api::account_read().send(&client)?.wait()?["account"].is_null() {
        return Err("Goal test must remain unauthenticated".into());
    }
    Ok((client, rx))
}

fn notified(
    rx: &mpsc::Receiver<Event>,
    thread: &str,
    expected: Option<&Goal>,
    turns: &mut Turns,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            Event::Notification { method, params } if method == "thread/goal/updated" => {
                let received = goals::decode(thread, &params)?;
                if !received
                    .as_ref()
                    .zip(expected)
                    .is_some_and(|(a, b)| a.same_configuration(b))
                {
                    // Accounting/status events already queued before the edit
                    // may arrive first. Never use them as its acknowledgement.
                    continue;
                }
                return Ok(());
            }
            Event::Notification { method, params } if method == "thread/goal/cleared" => {
                if expected.is_some() || params["threadId"] != thread {
                    return Err("Native clear notification has the wrong owner/state".into());
                }
                return Ok(());
            }
            event => turns.observe(event, thread)?,
        }
    }
}

#[derive(Default)]
struct Turns {
    started: std::collections::BTreeSet<String>,
    completed: std::collections::BTreeSet<String>,
}
impl Turns {
    fn observe(&mut self, event: Event, thread: &str) -> Result<()> {
        match event {
            Event::Closed { reason } => Err(reason.into()),
            Event::ServerRequest { .. } => {
                Err("Unexpected authority request; no approval sent".into())
            }
            Event::Notification { method, params }
                if method == "turn/started" || method == "turn/completed" =>
            {
                if params["threadId"] != thread {
                    return Err("Native goal ran on a sibling thread".into());
                }
                let id = params["turn"]["id"]
                    .as_str()
                    .ok_or("Missing native turn ID")?
                    .to_owned();
                if method == "turn/started" {
                    self.started.insert(id);
                } else {
                    if params["turn"]["status"] != "completed" {
                        return Err(format!("Native goal turn failed: {}", params["turn"]).into());
                    }
                    self.completed.insert(id);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn read(client: &Client, thread: &str) -> Result<Option<Goal>> {
    Ok(goals::decode(
        thread,
        &goals::get(thread).send(client)?.wait()?,
    )?)
}

fn edit(
    client: &Client,
    rx: &mpsc::Receiver<Event>,
    thread: &str,
    edit: Edit,
    turns: &mut Turns,
) -> Result<Goal> {
    let goal = goals::decode(thread, &edit.call(thread)?.send(client)?.wait()?)?
        .ok_or("Native edit returned no goal")?;
    notified(rx, thread, Some(&goal), turns)?;
    if !read(client, thread)?.is_some_and(|read| read.same_configuration(&goal)) {
        return Err("Native readback differs from acknowledged goal".into());
    }
    Ok(goal)
}

#[test]
#[ignore = "Explicit native active goal persistence probe; isolated profile, local endpoint, no account inference"]
fn native_goal_activation_and_paused_restart_use_server_execution() -> Result<()> {
    let mut fixture = reload_responses_fixture::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-goal-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"goal_fixture\"\n[model_providers.goal_fixture]\nname = \"Owned goal fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            fixture.address
        ),
    )?;
    let (mut client, mut rx) = connect(&native, &work)?;
    let result = (|| -> Result<()> {
        let mut threads = Vec::new();
        for _ in 0..2 {
            let response = api::start_thread(
                work.to_str().ok_or("Invalid cwd")?,
                &api::Profile::default(),
                api::Access::ReadOnly,
            )
            .send(&client)?
            .wait()?;
            if response["sandbox"]["type"] != "readOnly"
                || !response["thread"]["path"]
                    .as_str()
                    .is_some_and(|p| Path::new(p).starts_with(&native))
            {
                return Err("Thread escaped owned read-only scope".into());
            }
            threads.push(
                response["thread"]["id"]
                    .as_str()
                    .ok_or("Missing thread id")?
                    .to_owned(),
            );
        }
        let thread = &threads[0];
        let sibling = &threads[1];
        let mut turns = Turns::default();
        let original = edit(
            &client,
            &rx,
            thread,
            Edit::Replace {
                objective: "Isolated native goal state acceptance; no tools or files.".into(),
                status: UserStatus::Paused,
                token_budget: None,
            },
            &mut turns,
        )?;
        let activated = edit(
            &client,
            &rx,
            thread,
            Edit::Status {
                status: UserStatus::Active,
            },
            &mut turns,
        )?;
        if activated.status != Status::Active {
            return Err("Goal was not activated".into());
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while turns.started.is_empty() {
            turns.observe(
                rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))?,
                thread,
            )?;
        }
        // Pause changes future goal scheduling; it is not an interrupt of the
        // native turn already running. Let that fixed-text turn finish naturally.
        edit(
            &client,
            &rx,
            thread,
            Edit::Status {
                status: UserStatus::Paused,
            },
            &mut turns,
        )?;
        while turns.started != turns.completed {
            turns.observe(
                rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))?,
                thread,
            )?;
        }
        for (status, expected) in [
            (UserStatus::Blocked, Status::Blocked),
            (UserStatus::Paused, Status::Paused),
        ] {
            let updated = edit(&client, &rx, thread, Edit::Status { status }, &mut turns)?;
            if updated.status != expected
                || updated.created_at != original.created_at
                || updated.objective != original.objective
                || updated.token_budget.is_some()
                || read(&client, sibling)?.is_some()
            {
                return Err(
                    "Goal transition changed identity, accounting, budget or sibling".into(),
                );
            }
        }
        let persisted = read(&client, thread)?.ok_or("Missing paused goal")?;
        println!("Native activation and paused goal verified; restarting owned server.");
        client.shutdown();
        (client, rx) = connect(&native, &work)?;
        if read(&client, thread)?.as_ref() != Some(&persisted) {
            return Err("Native goal did not persist independently across server restart".into());
        }
        api::resume_thread(thread).send(&client)?.wait()?;
        let before_resume_observation = turns.started.len();
        // A bounded observation is evidence for this run, not a promise that
        // every future native runtime can never schedule autonomous work.
        let until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < until {
            match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(event) => turns.observe(event, thread)?,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(error) => return Err(error.into()),
            }
        }
        if turns.started != turns.completed || turns.started.len() != before_resume_observation {
            return Err("Paused goal started another turn after resume".into());
        }
        let history = api::read_thread(thread).send(&client)?.wait()?;
        let persisted_turns = history["thread"]["turns"]
            .as_array()
            .ok_or("Missing native history")?;
        if persisted_turns.len() != turns.started.len()
            || persisted_turns.iter().any(|turn| {
                turn["id"]
                    .as_str()
                    .is_none_or(|id| !turns.completed.contains(id))
            })
        {
            return Err("Native persisted history differs from observed goal turns".into());
        }
        let complete = edit(
            &client,
            &rx,
            thread,
            Edit::Status {
                status: UserStatus::Complete,
            },
            &mut turns,
        )?;
        if complete.status != Status::Complete {
            return Err("Completion was not applied".into());
        }
        if !goals::decode_clear(&goals::clear(thread).send(&client)?.wait()?)? {
            return Err("Existing native goal was not cleared".into());
        }
        notified(&rx, thread, None, &mut turns)?;
        // The unused sibling may have no persisted rollout in 0.153.4. Its
        // isolation was checked while loaded; do not invent history for it.
        if read(&client, thread)?.is_some() {
            return Err("Goal remained after clear".into());
        }
        api::delete_thread(thread).send(&client)?.wait()?;
        for event in rx.try_iter() {
            turns.observe(event, thread)?;
        }
        if fixture
            .requests
            .lock()
            .map_err(|_| "Fixture lock poisoned")?
            .is_empty()
        {
            return Err("Active native goal never reached the owned model fixture".into());
        }
        Ok(())
    })();
    client.shutdown();
    let finished = fixture.finish();
    result?;
    finished?;
    println!(
        "PASS: actual native goal activation started a native turn without turn/start; pause let the existing turn complete; blocked/complete/clear, matching notifications, sibling isolation and paused restart persistence passed. Local fixed-response model only; UI activation remains a separate acceptance gate."
    );
    Ok(())
}
