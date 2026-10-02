use super::*;

#[tokio::test]
async fn repeated_failed_sign_ins_lock_the_account_out() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut guesser = harness.client().await;
    let mut sign_in = async |password: &str| {
        guesser
            .send(|request| ClientMessage::Login {
                request,
                version: be_protocol::PROTOCOL_VERSION,
                email: "owner@example.com".into(),
                password: password.into(),
            })
            .await
    };

    for _ in 0..5 {
        let refused = sign_in("wrong password").await;
        assert!(
            matches!(&refused, ServerMessage::Failed { message, .. } if !message.contains("try again")),
            "{refused:?}"
        );
    }
    let locked = sign_in("correct horse battery").await;
    assert!(
        matches!(&locked, ServerMessage::Failed { message, .. } if message.contains("try again")),
        "the right password got in while the account was locked out: {locked:?}"
    );

    let mut other = harness.client().await;
    let response = other
        .send(|request| ClientMessage::Login {
            request,
            version: be_protocol::PROTOCOL_VERSION,
            email: "nobody@example.com".into(),
            password: "correct horse battery".into(),
        })
        .await;
    assert!(
        matches!(&response, ServerMessage::Failed { message, .. } if !message.contains("try again")),
        "another email was locked out too: {response:?}"
    );

    harness.stop().await;
}
