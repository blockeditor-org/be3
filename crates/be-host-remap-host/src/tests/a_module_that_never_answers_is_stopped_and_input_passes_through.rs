use std::time::Duration;

use super::*;

#[test]
fn a_module_that_never_answers_is_stopped_and_input_passes_through() {
    // It starts, then never answers input.
    let mut remapper = hostile_within(
        r#"
        (if (result i64) (i32.le_u (global.get $calls) (i32.const 1))
            (then (i64.const 0x400000000a))
            (else (loop $forever (br $forever)) (unreachable)))
        "#,
        Limits {
            call: Duration::from_millis(20),
            ..Limits::default()
        },
    );
    assert_eq!(remapper.started(), vec![]);

    assert_eq!(remapper.input(press("A"), NOW), vec![press("A")]);
    assert_eq!(
        remapper.take_problems(),
        vec![
            "The input remaps failed (they did not answer in time) and start again."
        ]
    );
    assert_eq!(remapper.input(release("A"), NOW), vec![release("A")]);
}
