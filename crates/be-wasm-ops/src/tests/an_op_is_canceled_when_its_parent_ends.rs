use super::*;

#[test]
fn an_op_is_canceled_when_its_parent_ends() {
    let mut host = Connection::new(Side::Host);
    let mut guest = Connection::new(Side::Guest);
    let resource = host.start(None, method_id("test#resource"), encode(&()));
    let method = host.start(Some(resource), method_id("test#method"), encode(&()));
    deliver(&mut host, &mut guest);
    events(&mut guest);
    let import = guest.start(Some(resource), method_id("test#import"), encode(&()));
    deliver(&mut guest, &mut host);
    assert_eq!(events(&mut host).len(), 1);

    guest.complete(resource, Outcome::returned(&()));
    assert_eq!(events(&mut guest), vec![Incoming::Canceled { op: method }]);
    deliver(&mut guest, &mut host);
    assert_eq!(
        events(&mut host),
        vec![
            Incoming::Completed {
                op: resource,
                outcome: Outcome::returned(&()),
            },
            Incoming::Canceled { op: import },
        ]
    );

    let late = host.start(Some(resource), method_id("test#method"), encode(&()));
    deliver(&mut host, &mut guest);
    assert!(events(&mut guest).is_empty());
    deliver(&mut guest, &mut host);
    assert!(events(&mut host).contains(&Incoming::Completed {
        op: late,
        outcome: Outcome::Err(OpError::Canceled),
    }));
}
