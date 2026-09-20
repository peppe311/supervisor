use central_agent_codex_runtime::{
    api,
    wire::{self, RequestId, Session},
};
fn main() {
    let mut calls = vec![Session::new(1).initialize("contract-test").unwrap()];
    calls.extend(
        api::contract_calls()
            .into_iter()
            .enumerate()
            .map(|(i, call)| {
                wire::request(&RequestId::Integer(i as i64 + 10), call.method, call.params)
            }),
    );
    println!("{}", serde_json::to_string(&calls).unwrap());
}
