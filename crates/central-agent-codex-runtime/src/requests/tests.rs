use super::*;

fn key(n: i64) -> ServerRequestKey {
    ServerRequestKey {
        generation: 7,
        id: RequestId::Integer(n),
    }
}
fn command() -> Value {
    json!({"threadId":"native-a","turnId":"turn-a","itemId":"cmd","command":"cargo test","cwd":"C:/project","_meta":{"secret":"must not enter UI"}})
}
fn ticket(requests: &Requests, owner: &str) -> String {
    requests.views(owner)[0].ticket.clone()
}

#[test]
fn pending_cards_keep_arrival_order_across_ticket_digit_boundaries() {
    let mut requests = Requests::default();
    let mut expected = [Vec::new(), Vec::new()];
    let owners = ["chat:a", "graph:b"];
    for index in 1..=60 {
        for (slot, owner) in owners.iter().enumerate() {
            // Native IDs are opaque and can sort in the opposite direction.
            // Multiple callbacks may share one item; that cannot merge cards.
            let native_key = ServerRequestKey {
                generation: 7,
                id: if slot == 0 {
                    RequestId::Integer(100 - index)
                } else {
                    RequestId::Text(format!("callback-{}", 100 - index))
                },
            };
            let mut params = command();
            params["threadId"] = json!(if slot == 0 { "native-a" } else { "native-b" });
            requests
                .insert(
                    owner,
                    native_key.clone(),
                    "item/commandExecution/requestApproval",
                    params,
                )
                .unwrap();
            let added = requests
                .pending
                .values()
                .find(|p| p.key == native_key)
                .unwrap()
                .view
                .ticket
                .clone();
            expected[slot].push(added);
            for (view_owner, order) in owners.iter().zip(&expected) {
                assert_eq!(
                    requests
                        .views(view_owner)
                        .iter()
                        .map(|v| &v.ticket)
                        .collect::<Vec<_>>(),
                    order.iter().collect::<Vec<_>>(),
                    "Existing cards moved when another request arrived"
                );
            }
        }
    }
    let selected = expected[0][4].clone();
    let (sent, response) = requests
        .answer("chat:a", &selected, Decision::Decline {})
        .unwrap();
    assert_eq!(sent.id, RequestId::Integer(95));
    assert_eq!(response, json!({"decision":"decline"}));
    assert_eq!(requests.views("chat:a")[4].ticket, selected);
    assert!(requests.views("chat:a")[4].responding);
    // Removal/late completion does not reorder survivors or touch the sibling.
    assert_eq!(requests.sent(&sent).as_deref(), Some("chat:a"));
    expected[0].remove(4);
    assert!(requests.sent(&sent).is_none());
    for (owner, order) in owners.iter().zip(&expected) {
        assert_eq!(
            requests
                .views(owner)
                .iter()
                .map(|v| &v.ticket)
                .collect::<Vec<_>>(),
            order.iter().collect::<Vec<_>>()
        );
    }
    assert!(
        requests
            .answer("graph:b", &selected, Decision::Accept {})
            .is_err()
    );
}

#[test]
fn closing_archiving_or_deleting_expires_only_that_threads_decisions() {
    for method in ["thread/closed", "thread/archived", "thread/deleted"] {
        let mut requests = Requests::default();
        requests
            .insert(
                "chat:a",
                key(1),
                "item/commandExecution/requestApproval",
                command(),
            )
            .unwrap();
        let mut other = command();
        other["threadId"] = json!("native-b");
        requests
            .insert(
                "graph:b",
                key(2),
                "item/commandExecution/requestApproval",
                other,
            )
            .unwrap();
        let old_ticket = ticket(&requests, "chat:a");
        assert_eq!(
            requests.notify(method, &json!({"threadId":"native-a"})),
            vec!["chat:a"]
        );
        assert!(
            requests
                .answer("chat:a", &old_ticket, Decision::Accept {})
                .is_err()
        );
        assert_eq!(requests.views("graph:b").len(), 1);
    }
}

#[test]
fn pending_decisions_are_scoped_to_owner_and_resolved_request_id_type() {
    let mut requests = Requests::default();
    requests
        .insert(
            "chat:a",
            key(1),
            "item/commandExecution/requestApproval",
            command(),
        )
        .unwrap();
    let id = ticket(&requests, "chat:a");
    assert!(requests.views("graph:b").is_empty());
    assert!(
        requests
            .answer("graph:b", &id, Decision::Accept {})
            .is_err()
    );
    assert!(
        requests
            .notify(
                "serverRequest/resolved",
                &json!({"threadId":"native-a","requestId":"1"})
            )
            .is_empty()
    );
    assert!(
        requests
            .notify(
                "serverRequest/resolved",
                &json!({"threadId":"foreign","requestId":1})
            )
            .is_empty()
    );
    assert_eq!(
        requests.notify(
            "serverRequest/resolved",
            &json!({"threadId":"native-a","requestId":1})
        ),
        vec!["chat:a"]
    );
    assert!(requests.answer("chat:a", &id, Decision::Accept {}).is_err());
}

#[test]
fn double_click_disconnect_and_late_send_cannot_approve_new_request() {
    let mut requests = Requests::default();
    requests
        .insert(
            "chat:a",
            key(1),
            "item/commandExecution/requestApproval",
            command(),
        )
        .unwrap();
    let id = ticket(&requests, "chat:a");
    let (old, response) = requests
        .answer("chat:a", &id, Decision::Decline {})
        .unwrap();
    assert_eq!(response, json!({"decision":"decline"}));
    assert!(requests.answer("chat:a", &id, Decision::Accept {}).is_err());
    assert_eq!(requests.disconnect(), vec!["chat:a"]);
    let next = ServerRequestKey {
        generation: 8,
        id: RequestId::Integer(1),
    };
    requests
        .insert(
            "chat:a",
            next,
            "item/commandExecution/requestApproval",
            command(),
        )
        .unwrap();
    assert!(requests.sent(&old).is_none());
    assert!(requests.answer("chat:a", &id, Decision::Accept {}).is_err());
    assert_eq!(requests.views("chat:a").len(), 1);
}

#[test]
fn completion_clears_only_its_turn_and_mcp_standalone_request_survives() {
    let mut requests = Requests::default();
    requests
        .insert(
            "chat:a",
            key(1),
            "item/commandExecution/requestApproval",
            command(),
        )
        .unwrap();
    requests.insert("chat:a",key(2),"mcpServer/elicitation/request",json!({"threadId":"native-a","turnId":null,"serverName":"test","mode":"url","url":"https://example.test"})).unwrap();
    assert_eq!(
        requests.notify(
            "turn/completed",
            &json!({"threadId":"native-a","turn":{"id":"turn-a"}})
        ),
        vec!["chat:a"]
    );
    assert_eq!(requests.views("chat:a").len(), 1);
    assert_eq!(requests.views("chat:a")[0].kind, Kind::Mcp);
}

#[test]
fn permission_grants_are_exactly_the_request_never_ui_supplied_paths() {
    let params =
        json!({"permissions":{"network":null,"fileSystem":{"read":["C:/one"],"write":null}}});
    assert_eq!(
        response(
            Kind::Permissions,
            &params,
            Decision::Permissions {
                grant: true,
                session: false
            }
        )
        .unwrap(),
        json!({"permissions":{"fileSystem":{"read":["C:/one"],"write":null}},"scope":"turn"})
    );
    assert_eq!(
        response(
            Kind::Permissions,
            &params,
            Decision::Permissions {
                grant: false,
                session: true
            }
        )
        .unwrap(),
        json!({"permissions":{},"scope":"turn"})
    );
    assert!(serde_json::from_value::<Decision>(json!({"kind":"permissions","grant":true,"session":false,"permissions":{"fileSystem":{"write":["C:/"]}}})).is_err());
    assert!(response(Kind::Permissions, &params, Decision::Accept {}).is_err());
}

#[test]
fn proposed_policy_is_chosen_from_server_not_constructed_by_ui() {
    let params = json!({"proposedExecpolicyAmendment":["cargo","test"],"proposedNetworkPolicyAmendments":[{"host":"example.test","action":"allow"}]});
    assert_eq!(
        response(Kind::Command, &params, Decision::ExecPolicy {}).unwrap(),
        json!({"decision":{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo","test"]}}})
    );
    assert!(response(Kind::Command, &params, Decision::NetworkPolicy { index: 1 }).is_err());
    assert!(response(Kind::Files, &params, Decision::ExecPolicy {}).is_err());
    assert!(response(Kind::Command, &json!({}), Decision::ExecPolicy {}).is_err());
}

#[test]
fn available_native_command_decisions_restrict_both_ui_and_wire() {
    let mut params = command();
    let amendment =
        json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo","test"]}});
    params["proposedExecpolicyAmendment"] = json!(["cargo", "test"]);
    params["availableDecisions"] = json!(["accept", amendment, "cancel"]);
    let mut requests = Requests::default();
    requests
        .insert(
            "chat:a",
            key(1),
            "item/commandExecution/requestApproval",
            params,
        )
        .unwrap();
    let view = requests.views("chat:a").pop().unwrap();
    let choices = view.choices.unwrap();
    assert!(choices.accept && choices.cancel && choices.exec_policy);
    assert!(!choices.session && !choices.decline);
    assert!(choices.network_policies.is_empty());
    assert!(view.params.get("availableDecisions").is_none());
    for decision in [Decision::Session {}, Decision::Decline {}] {
        assert!(requests.answer("chat:a", &view.ticket, decision).is_err());
        assert!(!requests.views("chat:a")[0].responding);
    }
    assert_eq!(
        requests
            .answer("chat:a", &view.ticket, Decision::ExecPolicy {})
            .unwrap()
            .1,
        json!({"decision":amendment})
    );
}

#[test]
fn native_choices_have_no_empty_malformed_or_unknown_fallback() {
    for available in [
        json!([]),
        json!([{"futureGrant":{"secret":"not projected"}}]),
        json!({}),
        json!(true),
    ] {
        let params = json!({"availableDecisions":available});
        let choices = approval_choices(Kind::Command, &params).unwrap();
        assert!(
            !choices.accept
                && !choices.decline
                && !choices.cancel
                && !choices.session
                && !choices.exec_policy
        );
        assert!(choices.network_policies.is_empty());
        assert!(response(Kind::Command, &params, Decision::Accept {}).is_err());
    }
    for params in [json!({}), json!({"availableDecisions":null})] {
        let choices = approval_choices(Kind::Command, &params).unwrap();
        assert!(choices.accept && choices.decline && choices.cancel && choices.session);
        assert!(!choices.exec_policy);
    }
    // This optional restriction belongs to command approvals, not file changes
    // or unrelated server requests with a coincidentally named future field.
    let files = approval_choices(Kind::Files, &json!({"availableDecisions":[]})).unwrap();
    assert!(files.accept && files.decline && files.cancel && files.session);
    assert!(approval_choices(Kind::Mcp, &json!({})).is_none());
}

#[test]
fn offered_policy_must_match_the_exact_proposal_not_only_its_variant() {
    let allow = json!({"host":"example.test","action":"allow"});
    let deny = json!({"host":"example.test","action":"deny"});
    let params = json!({"proposedExecpolicyAmendment":["cargo","test"],
        "proposedNetworkPolicyAmendments":[allow,deny],
        "availableDecisions":[{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["cargo"]}},
            {"applyNetworkPolicyAmendment":{"network_policy_amendment":deny}}]});
    let choices = approval_choices(Kind::Command, &params).unwrap();
    assert!(!choices.exec_policy);
    assert_eq!(choices.network_policies, vec![1]);
    assert!(response(Kind::Command, &params, Decision::ExecPolicy {}).is_err());
    assert!(response(Kind::Command, &params, Decision::NetworkPolicy { index: 0 }).is_err());
    assert_eq!(
        response(Kind::Command, &params, Decision::NetworkPolicy { index: 1 }).unwrap(),
        json!({"decision":{"applyNetworkPolicyAmendment":{"network_policy_amendment":deny}}})
    );
}

#[test]
fn unknown_requests_and_internal_metadata_never_become_ui_authority() {
    let mut requests = Requests::default();
    assert!(
        requests
            .insert("chat:a", key(1), "item/tool/call", command())
            .is_err()
    );
    assert!(
        requests
            .insert(
                "chat:a",
                key(1),
                "account/chatgptAuthTokens/refresh",
                command()
            )
            .is_err()
    );
    requests
        .insert(
            "chat:a",
            key(1),
            "item/commandExecution/requestApproval",
            command(),
        )
        .unwrap();
    assert!(
        !serde_json::to_string(&requests.views("chat:a"))
            .unwrap()
            .contains("must not enter UI")
    );
}

#[test]
fn questions_keep_exact_ids_and_no_unrequested_answers() {
    let params = json!({"questions":[{"id":"secret","question":"Password","isSecret":true},{"id":"mode","question":"Mode"}]});
    assert!(
        response(
            Kind::Questions,
            &params,
            Decision::Answers {
                answers: BTreeMap::from([("secret".into(), vec!["value".into()])])
            }
        )
        .is_err()
    );
    let result = response(
        Kind::Questions,
        &params,
        Decision::Answers {
            answers: BTreeMap::from([
                ("secret".into(), vec!["value".into()]),
                ("mode".into(), vec!["custom answer".into()]),
            ]),
        },
    )
    .unwrap();
    assert_eq!(
        result["answers"]["mode"],
        json!({"answers":["custom answer"]})
    );
    assert_eq!(
        response(Kind::Questions, &params, Decision::Cancel {}).unwrap(),
        json!({"answers":{}})
    );
}

#[test]
fn stable_mcp_form_validates_primitives_constraints_and_typed_choices() {
    let schema = json!({"type":"object","required":["count","mode"],"properties":{
        "count":{"type":"integer","minimum":1,"maximum":4},
        "mode":{"type":"string","oneOf":[{"const":"safe","title":"Safe"}]},
        "flags":{"type":"array","minItems":1,"items":{"anyOf":[{"const":"one"},{"const":"two"}]}},
        "enabled":{"type":"boolean"},"name":{"type":"string","minLength":2,"maxLength":4}
    }});
    assert!(
        validate_form(
            &schema,
            &json!({"count":2,"mode":"safe","flags":["one","two"],"enabled":false,"name":"test"})
        )
        .is_ok()
    );
    for bad in [
        json!({"count":1.5,"mode":"safe"}),
        json!({"count":5,"mode":"safe"}),
        json!({"count":2,"mode":"unsafe"}),
        json!({"count":2,"mode":"safe","flags":["one","one"]}),
        json!({"count":2,"mode":"safe","extra":true}),
        json!({"mode":"safe"}),
    ] {
        assert!(validate_form(&schema, &bad).is_err());
    }
    let params = json!({"mode":"form","requestedSchema":schema});
    assert_eq!(
        response(
            Kind::Mcp,
            &params,
            Decision::Elicitation {
                action: ElicitationAction::Decline,
                content: Some(json!({"secret":"discard"}))
            }
        )
        .unwrap(),
        json!({"action":"decline","content":null,"_meta":null})
    );
}
