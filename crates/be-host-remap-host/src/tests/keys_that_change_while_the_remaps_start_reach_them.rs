use super::*;

const CTRL_C: &str = r#"
    [[group]]
    rules = [{ on = "Ctrl+C", send = "X" }]
    "#;

#[test]
fn keys_that_change_while_the_remaps_start_reach_them() {
    // Ctrl pressed before `Start` is answered.
    let mut remapper = Remaps::new(MODULE, Limits::default());
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    remapper.started();
    assert!(
        remapper.input(press("C"), NOW).contains(&press("X")),
        "the remaps know Ctrl is held"
    );

    // Ctrl held as `Start` is sent and let go before it is answered.
    let mut remapper = Remaps::new(MODULE, Limits::default());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty());
    remapper.input(release("LeftCtrl"), NOW);
    remapper.started();
    assert_eq!(
        remapper.input(press("C"), NOW),
        vec![press("C")],
        "the remaps know Ctrl was let go"
    );
    assert!(remapper.take_problems().is_empty());
}
