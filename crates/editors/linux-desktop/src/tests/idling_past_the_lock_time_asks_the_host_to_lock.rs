use super::*;

#[test]
fn idling_past_the_lock_time_asks_the_host_to_lock() {
    let mut fixture = Fixture::new();
    fixture.allow_power(EVERYTHING);
    assert!(fixture.test.watches::<Idle>());
    assert!(fixture.test.take_actions::<PowerAction>().is_empty());

    fixture
        .test
        .set_host_value::<Idle>(&IdleState { lock_due: true });
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<PowerAction>(),
        vec![PowerAction::Lock]
    );
    fixture.settle();
    assert!(
        fixture.test.take_actions::<PowerAction>().is_empty(),
        "one idle spell asks once"
    );

    fixture
        .test
        .set_host_value::<Idle>(&IdleState { lock_due: false });
    fixture.settle();
    fixture
        .test
        .set_host_value::<Idle>(&IdleState { lock_due: true });
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<PowerAction>(),
        vec![PowerAction::Lock],
        "the next one asks again"
    );
}
