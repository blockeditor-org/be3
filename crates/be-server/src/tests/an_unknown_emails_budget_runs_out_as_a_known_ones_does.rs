use super::*;
use crate::store::codes::{self, CodePurpose, CodeRequest};

#[tokio::test]
async fn an_unknown_emails_budget_runs_out_as_a_known_ones_does() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    // Strangers, each from a network of their own, ask for both emails as often as each budget
    // allows, and once more.
    let mut answers = Vec::new();
    for email in ["owner@example.com", "nobody@example.com"] {
        let mut refused_at = None;
        for stranger in 0..codes::CODES_PER_ACCOUNT + 1 {
            let address = format!("2001:db8:{:x}::1", 0x1000 + stranger);
            let attempt = crate::new_secret();
            let answer = store
                .request_code(CodeRequest {
                    purpose: CodePurpose::SignIn,
                    email,
                    account: None,
                    display_name: "",
                    invite: "",
                    device_code: "",
                    attempt: &attempt,
                    address: &address,
                })
                .await;
            if let Err(error) = answer {
                refused_at.get_or_insert((stranger, error.to_string()));
            }
        }
        answers.push(refused_at);
    }
    assert!(answers[0].is_some());
    assert_eq!(
        answers[0], answers[1],
        "an email with no account runs out at the same request, with the same answer"
    );
}
