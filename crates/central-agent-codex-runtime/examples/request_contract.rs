//! Validate the real reply builder against each selected runtime response schema.
use central_agent_codex_runtime::{
    requests::{Decision, Requests},
    wire::{RequestId, ServerRequestKey},
};
use serde_json::{Value, json};
fn main() {
    let mut examples = Vec::new();
    let scopes = json!({"threadId":"thread-a","turnId":"turn-a","itemId":"item-a","startedAtMs":1,"environmentId":null});
    let cases = [
        (
            "item/commandExecution/requestApproval",
            "CommandExecutionRequestApprovalResponse",
            json!({"kind":"command","command":"cargo test","cwd":"C:/project","proposedExecpolicyAmendment":["cargo","test"],"proposedNetworkPolicyAmendments":[{"host":"example.test","action":"allow"}]}),
            vec![
                json!({"kind":"accept"}),
                json!({"kind":"session"}),
                json!({"kind":"decline"}),
                json!({"kind":"cancel"}),
                json!({"kind":"exec_policy"}),
                json!({"kind":"network_policy","index":0}),
            ],
        ),
        (
            "item/fileChange/requestApproval",
            "FileChangeRequestApprovalResponse",
            json!({}),
            vec![
                json!({"kind":"accept"}),
                json!({"kind":"session"}),
                json!({"kind":"decline"}),
                json!({"kind":"cancel"}),
            ],
        ),
        (
            "item/permissions/requestApproval",
            "PermissionsRequestApprovalResponse",
            json!({"cwd":"C:/project","reason":null,"permissions":{"network":{"enabled":true},"fileSystem":{"read":["C:/project"],"write":null}}}),
            vec![
                json!({"kind":"permissions","grant":true,"session":false}),
                json!({"kind":"permissions","grant":true,"session":true}),
                json!({"kind":"permissions","grant":false,"session":false}),
            ],
        ),
        (
            "item/tool/requestUserInput",
            "ToolRequestUserInputResponse",
            json!({"questions":[{"id":"q","header":"Choice","question":"Choose a direction","isOther":true,"isSecret":false,"options":null}],"isBlocking":true,"autoResolutionMs":null}),
            vec![
                json!({"kind":"answers","answers":{"q":["Read only"]}}),
                json!({"kind":"cancel"}),
            ],
        ),
        (
            "mcpServer/elicitation/request",
            "McpServerElicitationRequestResponse",
            json!({"serverName":"fixture","mode":"form","_meta":null,"message":"Select","requestedSchema":{"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}}),
            vec![
                json!({"kind":"elicitation","action":"accept","content":{"enabled":false}}),
                json!({"kind":"elicitation","action":"decline","content":null}),
                json!({"kind":"elicitation","action":"cancel","content":null}),
            ],
        ),
        (
            "mcpServer/elicitation/request",
            "McpServerElicitationRequestResponse",
            json!({"turnId":null,"serverName":"fixture","mode":"url","_meta":null,"message":"Authorize","url":"https://example.test/authorize","elicitationId":"auth-a"}),
            vec![json!({"kind":"elicitation","action":"accept","content":null})],
        ),
    ];
    for (method, schema, extra, decisions) in cases {
        let mut params = scopes.clone();
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        for decision in decisions {
            let mut requests = Requests::default();
            requests
                .insert(
                    "chat:a",
                    ServerRequestKey {
                        generation: 1,
                        id: RequestId::Integer(2),
                    },
                    method,
                    params.clone(),
                )
                .unwrap();
            let ticket = requests.views("chat:a")[0].ticket.clone();
            let decision: Decision = serde_json::from_value(decision).unwrap();
            let (_, result) = requests.answer("chat:a", &ticket, decision).unwrap();
            examples.push(json!({"schema":schema,"request":{"id":2,"method":method,"params":params},"result":result}));
        }
    }
    println!("{}", Value::Array(examples));
}
