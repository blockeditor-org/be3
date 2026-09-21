use super::*;

#[test]
fn a_counter_round_trips_through_its_bytes() {
    let counter = CounterContent::new(-9_000);

    let bytes = counter.encode();

    assert_eq!(bytes.len(), 8);
    assert_eq!(CounterContent::decode(&bytes), Ok(counter));
    assert_eq!(
        CounterContent::decode(&bytes[..4]),
        Err(ContentError::Malformed("a counter is eight bytes"))
    );
}
