//! Native goal data and typed UI edits. No scheduling, inference or persistence engine.
use crate::api::Call;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub thread_id: String,
    pub objective: String,
    pub status: Status,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: i64,
    pub updated_at: i64,
}
impl Goal {
    /// Live accounting updates do not change confirmation authority. Native goal
    /// replacement/status/budget changes do; no native compare-and-swap is claimed.
    pub fn same_configuration(&self, other: &Self) -> bool {
        self.thread_id == other.thread_id
            && self.objective == other.objective
            && self.status == other.status
            && self.token_budget == other.token_budget
            && self.created_at == other.created_at
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UserStatus {
    Active,
    Paused,
    Blocked,
    Complete,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Replace {
        objective: String,
        status: UserStatus,
        #[serde(deserialize_with = "Option::deserialize")]
        token_budget: Option<u64>,
    },
    Status {
        status: UserStatus,
    },
    Budget {
        #[serde(deserialize_with = "Option::deserialize")]
        token_budget: Option<u64>,
    },
    Clear {},
}
impl Edit {
    pub fn call(&self, thread: &str) -> Result<Call, String> {
        if thread.is_empty() {
            return Err("Select a native conversation".into());
        }
        let mut params = json!({"threadId":thread});
        match self {
            Self::Replace {
                objective,
                status,
                token_budget,
            } => {
                if objective.trim().is_empty()
                    || objective.chars().count() > 4000
                    || objective.contains('\0')
                {
                    return Err("Specify an objective of 1–4,000 characters".into());
                }
                validate_budget(*token_budget)?;
                params["objective"] = json!(objective);
                params["status"] = json!(status);
                params["tokenBudget"] = json!(token_budget);
            }
            Self::Status { status } => params["status"] = json!(status),
            Self::Budget { token_budget } => {
                validate_budget(*token_budget)?;
                params["tokenBudget"] = json!(token_budget);
            }
            Self::Clear {} => return Ok(clear(thread)),
        }
        Ok(Call {
            method: "thread/goal/set",
            params,
        })
    }
}
fn validate_budget(budget: Option<u64>) -> Result<(), String> {
    if budget.is_some_and(|budget| budget == 0 || budget > 9_007_199_254_740_991) {
        return Err("The optional token budget must be a positive safe integer".into());
    }
    Ok(())
}
pub fn get(thread: &str) -> Call {
    Call {
        method: "thread/goal/get",
        params: json!({"threadId":thread}),
    }
}
pub fn clear(thread: &str) -> Call {
    Call {
        method: "thread/goal/clear",
        params: json!({"threadId":thread}),
    }
}
/// False means the goal was already absent, not a failed deletion.
pub fn decode_clear(value: &Value) -> Result<bool, String> {
    value
        .get("cleared")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Native goal clear response is missing its boolean acknowledgement".into())
}
pub fn decode(thread: &str, value: &Value) -> Result<Option<Goal>, String> {
    let raw = value
        .get("goal")
        .ok_or("Native goal response is missing goal")?;
    if raw.is_null() {
        return Ok(None);
    }
    let goal: Goal =
        serde_json::from_value(raw.clone()).map_err(|_| "Native goal response is malformed")?;
    if goal.thread_id != thread {
        return Err("Native goal belongs to another conversation".into());
    }
    Ok(Some(goal))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_do_not_replace_objectives_or_set_budgets_implicitly() {
        assert_eq!(
            Edit::Status {
                status: UserStatus::Paused
            }
            .call("a")
            .unwrap()
            .params,
            json!({"threadId":"a","status":"paused"})
        );
        assert_eq!(
            Edit::Budget { token_budget: None }
                .call("a")
                .unwrap()
                .params,
            json!({"threadId":"a","tokenBudget":null})
        );
        for budget in [Some(0), Some(u64::MAX)] {
            assert!(
                Edit::Budget {
                    token_budget: budget
                }
                .call("a")
                .is_err()
            );
        }
        assert!(
            Edit::Replace {
                objective: " ".into(),
                status: UserStatus::Paused,
                token_budget: None
            }
            .call("a")
            .is_err()
        );
        assert!(
            Edit::Replace {
                objective: "😀".repeat(4000),
                status: UserStatus::Paused,
                token_budget: None
            }
            .call("a")
            .is_ok()
        );
        assert!(
            Edit::Replace {
                objective: "a".repeat(4001),
                status: UserStatus::Paused,
                token_budget: None
            }
            .call("a")
            .is_err()
        );
        assert!(
            serde_json::from_value::<Edit>(json!({"kind":"status","status":"usageLimited"}))
                .is_err()
        );
    }
    #[test]
    fn missing_or_foreign_goal_is_not_an_empty_goal() {
        assert!(decode_clear(&json!({"cleared":true})).unwrap());
        assert!(!decode_clear(&json!({"cleared":false})).unwrap());
        assert!(decode_clear(&json!({})).is_err());
        assert!(serde_json::from_value::<Edit>(json!({"kind":"budget"})).is_err());
        assert!(decode("a", &json!({})).is_err());
        assert_eq!(decode("a", &json!({"goal":null})).unwrap(), None);
        let goal = json!({"threadId":"b","objective":"x","status":"paused","tokenBudget":null,"tokensUsed":0,"timeUsedSeconds":0,"createdAt":1,"updatedAt":1});
        assert!(decode("a", &json!({"goal":goal})).is_err());
        let a = decode("b", &json!({"goal":goal})).unwrap().unwrap();
        let mut b = a.clone();
        b.tokens_used = 5;
        b.updated_at = 2;
        assert!(a.same_configuration(&b));
        b.status = Status::Complete;
        assert!(!a.same_configuration(&b));
    }
}
