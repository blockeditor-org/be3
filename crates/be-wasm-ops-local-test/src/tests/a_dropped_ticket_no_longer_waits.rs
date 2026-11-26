use super::*;

#[test]
fn a_dropped_ticket_no_longer_waits() {
    let settling = Settling::default();
    let ticket = settling.ticket::<u32, be_wasm_ops::wire::Plain>("grow", 1);
    let items = settling.items::<u32, be_wasm_ops::wire::Plain>("errors", 2);
    assert!(settling.waiting());
    drop(ticket);
    assert!(settling.waiting(), "the stream still waits");
    drop(items);
    assert!(!settling.waiting(), "nothing waits once both are dropped");
    // What settles for them later is ignored.
    settling.settle(vec![
        Promise::<u32>::new(1).settle(&3),
        Sender::<u32>::new(2).end(),
    ]);
}
