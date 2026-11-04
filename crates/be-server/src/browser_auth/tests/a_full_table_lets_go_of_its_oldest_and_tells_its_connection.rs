use super::*;

/// Filling the server's pending sign-ins or forms never refuses a newcomer: the oldest entry is
/// let go of. A connection whose pending sign-in is let go of hears so, which ends the longer
/// time it was given to sign in.
#[tokio::test]
async fn a_full_table_lets_go_of_its_oldest_and_tells_its_connection() {
    let mut table = Bounded::new(4, 2);
    for (index, last) in [1u8, 2, 3, 4, 5, 6].into_iter().enumerate() {
        table.insert(index.to_string(), index, IpAddr::from([192, 0, 2, last]));
    }
    assert_eq!(table.len(), 4);
    assert!(table.get("0").is_none() && table.get("1").is_none());
    assert_eq!(table.get("5"), Some(&5), "the newest entry was refused");
    let busy = IpAddr::from([198, 51, 100, 1]);
    table.insert("a".into(), 10, busy);
    table.insert("b".into(), 11, busy);
    table.insert("c".into(), 12, busy);
    assert!(
        table.get("a").is_none() && table.get("c").is_some(),
        "an address held more than it may"
    );
    assert_eq!(table.len(), 4);

    let sessions = Arc::new(Sessions::default());
    let address = IpAddr::from([192, 0, 2, 1]);
    let (_, _, mut first) = sessions.begin(1, address, None).unwrap();
    assert!(!first.ended());
    for client in 2..=PENDING_PER_ADDRESS as u64 {
        sessions.begin(client, address, None).unwrap();
    }
    assert!(!first.ended(), "a sign-in ended before its address was full");
    sessions.begin(100, address, None).unwrap();
    assert!(
        first.ended(),
        "the connection whose sign-in was let go of still waits on it"
    );

    let (code, secret, mut began) = sessions.begin(200, IpAddr::from([203, 0, 113, 1]), None).unwrap();
    let Ok(Finished::Waiting(waiting)) = sessions.finish(201, &code, &secret) else {
        panic!("another connection could not wait on the sign-in");
    };
    let mut taken = waiting.signing_in.expect("the sign-in moved to the connection waiting");
    assert!(
        began.ended(),
        "the connection that began a sign-in another took over still waits on it"
    );
    assert!(!taken.ended());
    let (_, _, again) = sessions.begin(200, IpAddr::from([203, 0, 113, 1]), Some(&code)).unwrap();
    assert!(
        !taken.ended(),
        "a connection let go of a sign-in another waits on"
    );
    drop(again);
}
