use super::*;

#[test]
fn randomness_outside_the_memory_stops_the_guest() {
    let answered = Wasi::new([0; 32]).call("random_get", &[60, 8], &mut Bytes(vec![0; 64]), &mut |_| {});
    assert!(matches!(answered, Err(Stopped::Fault(_))), "{answered:?}");
}
