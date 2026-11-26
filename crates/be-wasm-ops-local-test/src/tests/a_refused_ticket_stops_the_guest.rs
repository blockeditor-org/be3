use super::*;

#[test]
#[should_panic(expected = "ready was refused by the host")]
fn a_refused_ticket_stops_the_guest() {
    let settling = Settling::default();
    let _ticket =
        settling.ticket::<(), be_wasm_ops::wire::Plain>("ready", be_wasm_ops::local::REFUSED);
}
