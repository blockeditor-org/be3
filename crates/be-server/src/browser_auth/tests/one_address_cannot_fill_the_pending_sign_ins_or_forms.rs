use super::*;

#[tokio::test]
async fn one_address_cannot_fill_the_pending_sign_ins_or_forms() {
    let sessions = Sessions::default();
    let busy = IpAddr::from([192, 0, 2, 1]);
    let (oldest, oldest_secret, _, _) = sessions.begin(busy);
    for _ in 0..2 * PENDING_PER_ADDRESS {
        sessions.begin(busy);
    }
    assert_eq!(
        sessions.pending.lock().unwrap().len(),
        PENDING_PER_ADDRESS,
        "an address holds only so many pending sign-ins"
    );
    assert!(
        matches!(sessions.poll(&oldest, &oldest_secret), Ok(Polled::Expired)),
        "beginning another lets go of the address's oldest"
    );
    let (other, other_secret, _, _) = sessions.begin(IpAddr::from([198, 51, 100, 1]));
    assert!(
        matches!(sessions.poll(&other, &other_secret), Ok(Polled::Pending)),
        "another address still begins a sign-in"
    );

    let first_form = sessions.form("", busy);
    for _ in 0..2 * FORMS_PER_ADDRESS {
        sessions.form("", busy);
    }
    assert_eq!(sessions.forms.lock().unwrap().len(), FORMS_PER_ADDRESS);
    assert!(sessions.consume_form(&first_form, "").is_err());
    let other_form = sessions.form("", IpAddr::from([198, 51, 100, 1]));
    assert!(sessions.consume_form(&other_form, "").is_ok());
}
