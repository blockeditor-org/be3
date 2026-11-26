use super::*;

#[test]
fn a_ticket_dropped_before_it_settles_is_named_to_the_host_once() {
    let mut caller = Caller::new();
    let settling = Settling::default();
    let kept = settling.ticket::<u32, be_wasm_ops::wire::Plain>("kept", 1);
    let given_up = settling.ticket::<u32, be_wasm_ops::wire::Plain>("given up", 2);
    let mut settled = settling.ticket::<u32, be_wasm_ops::wire::Plain>("settled", 3);
    let items = settling.items::<u32, be_wasm_ops::wire::Plain>("errors", 4);
    caller.host.settled.push(Promise::<u32>::new(3).settle(&7));
    settle(&mut caller, &settling);
    assert_eq!(settled.take(), Some(7));
    drop(settled);
    drop(given_up);
    drop(items);
    settle(&mut caller, &settling);
    assert_eq!(
        caller.host.dropped,
        vec![2, 4],
        "the next poll names the unsettled ticket and the unended stream, not the settled one"
    );
    settle(&mut caller, &settling);
    assert_eq!(caller.host.dropped, vec![2, 4], "each is named once");
    assert!(settling.waiting(), "the kept ticket still waits");
    drop(kept);
    settle(&mut caller, &settling);
    assert_eq!(caller.host.dropped, vec![2, 4, 1]);
    assert!(!settling.waiting());
}
