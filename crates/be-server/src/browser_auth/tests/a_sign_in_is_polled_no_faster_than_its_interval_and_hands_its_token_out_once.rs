use super::*;

#[tokio::test(start_paused = true)]
async fn a_sign_in_is_polled_no_faster_than_its_interval_and_hands_its_token_out_once() {
    let sessions = Sessions::default();
    let address = IpAddr::from([192, 0, 2, 1]);
    let (code, secret, interval, expires) = sessions.begin(address);
    assert_eq!((interval, expires), (POLL_INTERVAL, LIFETIME));
    assert!(matches!(sessions.poll(&code, &secret), Ok(Polled::Pending)));
    let soon = sessions.poll(&code, &secret);
    assert!(
        matches!(soon, Err(ServerError::Refused(ErrorCode::SlowDown, _))),
        "a poll sooner than the interval was answered"
    );
    tokio::time::advance(POLL_INTERVAL).await;
    assert!(
        matches!(
            sessions.poll(&code, &secret),
            Err(ServerError::Refused(ErrorCode::SlowDown, _))
        ),
        "polling too soon did not lengthen the interval"
    );
    tokio::time::advance(POLL_INTERVAL + 2 * SLOW_DOWN).await;
    assert!(matches!(sessions.poll(&code, &secret), Ok(Polled::Pending)));

    let account = Uuid::new_v4();
    {
        let mut pending = sessions.pending.lock().unwrap();
        let entry = pending.get_mut(&code).unwrap();
        entry.account = Some(account);
        entry.approved_by = "a browser session".into();
    }
    tokio::time::advance(POLL_INTERVAL + 2 * SLOW_DOWN).await;
    assert!(matches!(
        sessions.poll(&code, &secret),
        Ok(Polled::Approved { account: approved, .. }) if approved == account
    ));
    tokio::time::advance(POLL_INTERVAL + 2 * SLOW_DOWN).await;
    assert!(
        matches!(sessions.poll(&code, &secret), Ok(Polled::Expired)),
        "an approval was handed out twice"
    );

    let (denied, denied_secret, _, _) = sessions.begin(address);
    sessions
        .pending
        .lock()
        .unwrap()
        .get_mut(&denied)
        .unwrap()
        .denied = true;
    assert!(matches!(
        sessions.poll(&denied, &denied_secret),
        Ok(Polled::Denied)
    ));
    tokio::time::advance(POLL_INTERVAL).await;
    assert!(matches!(
        sessions.poll(&denied, &denied_secret),
        Ok(Polled::Expired)
    ));
}
