use super::*;

#[tokio::test(start_paused = true)]
async fn cancelled_and_expired_authorizations_cannot_be_claimed() {
    let sessions = Sessions::default();
    let address = IpAddr::from([192, 0, 2, 1]);
    let (code, secret, _, _) = sessions.begin(address);
    assert!(
        matches!(sessions.poll(&code, "not the secret"), Ok(Polled::Expired)),
        "a wrong secret read a sign-in"
    );
    let (expired, expired_secret, _, _) = sessions.begin(address);
    sessions
        .pending
        .lock()
        .unwrap()
        .get_mut(&expired)
        .unwrap()
        .account = Some(Uuid::new_v4());
    tokio::time::advance(LIFETIME + Duration::from_secs(1)).await;
    assert!(matches!(
        sessions.poll(&expired, &expired_secret),
        Ok(Polled::Expired)
    ));
    assert!(matches!(sessions.poll(&code, &secret), Ok(Polled::Expired)));
    let form = sessions.form("", address);
    tokio::time::advance(LIFETIME + Duration::from_secs(1)).await;
    assert!(sessions.consume_form(&form, "").is_err());
}
