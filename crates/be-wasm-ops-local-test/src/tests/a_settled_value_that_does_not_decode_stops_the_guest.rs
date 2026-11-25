use super::*;

#[test]
#[should_panic(expected = "lost settled with what does not decode")]
fn a_settled_value_that_does_not_decode_stops_the_guest() {
    let settling = Settling::default();
    let _lost = settling.ticket::<String, be_wasm_ops::wire::Plain>("lost", 1);
    // A string whose length runs past its bytes.
    settling.settle(vec![Settled {
        ticket: 1,
        value: Some(vec![9, b'x']),
    }]);
}
