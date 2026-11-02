use super::*;

#[test]
fn frame_round_trips() {
    let message = hello();
    round_trips(&message);
}
