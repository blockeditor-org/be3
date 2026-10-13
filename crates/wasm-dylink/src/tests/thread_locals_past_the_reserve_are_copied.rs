use super::*;

#[test]
fn thread_locals_past_the_reserve_are_copied() {
    let mut layout = plan(
        Declared::of(["__be3_dylink_0_0_0_0"]).unwrap(),
        8,
        0,
        [],
        &sdk(32),
    )
    .unwrap();
    layout.relocated(500, 32);
    assert_eq!(layout.thread_locals(900), Ok(None));
    layout.relocated(500, 48);
    assert_eq!(
        layout.thread_locals(900),
        Ok(Some(Image {
            from: 532,
            to: 932,
            length: 16,
        }))
    );
}
