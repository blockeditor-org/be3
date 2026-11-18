use super::*;

/// One network (a /48) holds many /64s, each with its own share: past the network's own limit
/// it lets go of its oldest, so it never pushes out anyone else's sign-ins or forms.
#[tokio::test]
async fn one_network_cannot_fill_the_pending_sign_ins_or_forms() {
    let sessions = Sessions::default();
    let elsewhere = IpAddr::from([198, 51, 100, 1]);
    let (code, secret, _, _) = sessions.begin(elsewhere);
    let form = sessions.form("", elsewhere, None);
    // Each of the network's /64s, as many as fill the server's table.
    let in_network = |index: usize| {
        IpAddr::from(std::net::Ipv6Addr::new(
            0x2001,
            0xdb8,
            0x1,
            index as u16,
            0,
            0,
            0,
            1,
        ))
    };
    for index in 0..=LIMIT / PENDING_PER_ADDRESS {
        for _ in 0..PENDING_PER_ADDRESS {
            sessions.begin(in_network(index));
        }
    }
    assert!(
        matches!(sessions.poll(&code, &secret), Ok(Polled::Pending)),
        "one network pushed out another's sign-in"
    );
    assert_eq!(
        sessions.pending.lock().unwrap().len(),
        PENDING_PER_NETWORK + 1
    );
    for index in 0..=LIMIT / FORMS_PER_ADDRESS {
        for _ in 0..FORMS_PER_ADDRESS {
            sessions.form("", in_network(index), None);
        }
    }
    assert_eq!(sessions.forms.lock().unwrap().len(), FORMS_PER_NETWORK + 1);
    assert!(
        sessions.consume_form(&form, "").is_ok(),
        "one network pushed out another's form"
    );
}
