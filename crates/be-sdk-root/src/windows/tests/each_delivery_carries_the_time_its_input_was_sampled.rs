use super::*;

#[test]
fn each_delivery_carries_the_time_its_input_was_sampled() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    let delivered = windows.route(
        None,
        vec![
            Timed {
                input: button(150.0, true),
                time: Some(7_000),
            },
            Timed {
                input: Input::PointerMoved(pos2(160.0, 50.0)),
                time: Some(8_000),
            },
            button(150.0, false).into(),
        ],
    );
    let timed: Vec<(Input, Option<u64>)> = delivered
        .into_iter()
        .map(|delivery| (delivery.input, delivery.time))
        .collect();
    assert_eq!(
        timed,
        vec![
            (Input::Focus(true), Some(7_000)),
            (button(50.0, true), Some(7_000)),
            (Input::PointerMoved(pos2(60.0, 50.0)), Some(8_000)),
            (button(50.0, false), None),
        ],
        "what each input routes to carries its time, the focus its press made too, and an input \
         the platform did not time carries none"
    );
}
