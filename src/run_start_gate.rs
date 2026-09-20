use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

type RunStartGateState = (Mutex<Option<Result<(), String>>>, Condvar);

/// Coordinates asynchronous pre-run work with a provider worker.
///
/// The provider process is not spawned until the gate resolves successfully. The
/// wait has no deadline, but checks the provider's cancellation signal so a run
/// can still be stopped while its pre-run work is in progress.
#[derive(Clone, Debug)]
pub(crate) struct RunStartGate {
    state: Arc<RunStartGateState>,
}

impl RunStartGate {
    pub(crate) fn pending() -> Self {
        Self {
            state: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }

    pub(crate) fn resolve(&self, result: Result<(), String>) {
        let (state, changed) = &*self.state;
        if let Ok(mut state) = state.lock() {
            *state = Some(result);
            changed.notify_all();
        }
    }

    pub(crate) fn wait<F>(&self, is_cancelled: F) -> Result<(), String>
    where
        F: Fn() -> bool,
    {
        let (state, changed) = &*self.state;
        let mut state = state
            .lock()
            .map_err(|_| "The run start gate is unavailable".to_owned())?;
        loop {
            if is_cancelled() {
                return Err("Run stopped".to_owned());
            }
            if let Some(result) = state.as_ref() {
                return result.clone();
            }
            let (next, _) = changed
                .wait_timeout(state, Duration::from_millis(100))
                .map_err(|_| "The run start gate is unavailable".to_owned())?;
            state = next;
        }
    }
}

#[derive(Default)]
pub(crate) struct RunStartGateRegistry {
    gates: HashMap<u64, RunStartGate>,
}

impl RunStartGateRegistry {
    pub(crate) fn insert(&mut self, run_id: u64, gate: RunStartGate) {
        debug_assert!(self.gates.insert(run_id, gate).is_none());
    }

    /// Taking the gate transfers the single right to finalize its prerequisite.
    pub(crate) fn take_for_finalization(&mut self, run_id: u64) -> Option<RunStartGate> {
        self.gates.remove(&run_id)
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use super::{RunStartGate, RunStartGateRegistry};

    #[test]
    fn resolved_gate_returns_the_pre_run_result() {
        let gate = RunStartGate::pending();
        gate.resolve(Err("checkpoint failed".to_owned()));

        assert_eq!(gate.wait(|| false), Err("checkpoint failed".to_owned()));
    }

    #[test]
    fn pending_gate_can_be_cancelled() {
        let gate = RunStartGate::pending();

        assert_eq!(gate.wait(|| true), Err("Run stopped".to_owned()));
    }

    #[test]
    fn failed_prerequisite_keeps_provider_launch_fail_closed() {
        let gate = RunStartGate::pending();
        gate.resolve(Err("checkpoint baseline failed".to_owned()));

        assert_eq!(
            gate.wait(|| false),
            Err("checkpoint baseline failed".to_owned())
        );
    }

    #[test]
    fn finalization_waits_for_start_and_can_only_claim_a_gate_once() {
        let gate = RunStartGate::pending();
        let mut registry = RunStartGateRegistry::default();
        registry.insert(7, gate.clone());
        let finalizer_gate = registry.take_for_finalization(7).unwrap();
        assert!(registry.take_for_finalization(7).is_none());

        let (finished_tx, finished_rx) = mpsc::channel();
        thread::spawn(move || {
            finalizer_gate.wait(|| false).unwrap();
            finished_tx.send(()).unwrap();
        });
        assert!(finished_rx.recv_timeout(Duration::from_millis(20)).is_err());

        gate.resolve(Ok(()));
        finished_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("the finalizer should resume after checkpoint start");
    }
}
