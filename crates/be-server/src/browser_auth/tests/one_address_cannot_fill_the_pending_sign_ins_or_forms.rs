use super::*;

#[tokio::test]
async fn one_address_cannot_fill_the_pending_sign_ins_or_forms() {
    let sessions = Arc::new(Sessions::default());
    let busy = IpAddr::from([192, 0, 2, 1]);
    let (oldest, oldest_secret, _) = sessions.begin(0, busy, None).unwrap();
    for client in 1..=2 * PENDING_PER_ADDRESS as u64 {
        sessions.begin(client, busy, None).unwrap();
    }
    assert_eq!(
        sessions.pending.lock().unwrap().len(),
        PENDING_PER_ADDRESS,
        "an address holds only so many pending sign-ins"
    );
    assert!(
        sessions.finish(0, &oldest, &oldest_secret).is_err(),
        "beginning another lets go of the address's oldest"
    );
    let (other, other_secret, _) = sessions
        .begin(100, IpAddr::from([198, 51, 100, 1]), None)
        .unwrap();
    assert!(
        matches!(
            sessions.finish(100, &other, &other_secret),
            Ok(Finished::Waiting(_))
        ),
        "another address still begins a sign-in"
    );

    let first_form = sessions.form("", busy).unwrap();
    for _ in 0..2 * FORMS_PER_ADDRESS {
        sessions.form("", busy).unwrap();
    }
    assert_eq!(sessions.forms.lock().unwrap().len(), FORMS_PER_ADDRESS);
    assert!(sessions.consume_form(&first_form, "").is_err());
    let other_form = sessions.form("", IpAddr::from([198, 51, 100, 1])).unwrap();
    assert!(sessions.consume_form(&other_form, "").is_ok());
}
