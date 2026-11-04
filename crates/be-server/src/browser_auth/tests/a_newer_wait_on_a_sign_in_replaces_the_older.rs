use super::*;

#[tokio::test]
async fn a_newer_wait_on_a_sign_in_replaces_the_older() {
    let sessions = Arc::new(Sessions::default());
    let (code, secret, _) = sessions
        .begin(1, IpAddr::from([192, 0, 2, 1]), None)
        .unwrap();
    let Ok(Finished::Waiting(mut older)) = sessions.finish(1, &code, &secret) else {
        panic!("the sign-in is not decided yet");
    };
    assert!(matches!(
        older.replaced.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    let Ok(Finished::Waiting(mut newer)) = sessions.finish(1, &code, &secret) else {
        panic!("the sign-in is not decided yet");
    };
    assert!(
        matches!(
            older.replaced.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ),
        "the older wait is told to stop"
    );
    assert!(matches!(
        newer.replaced.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}
