use super::*;

#[test]
fn a_failure_releases_what_the_remaps_held_down() {
    let mut remapper = hostile(
        r#"
        (if (result i64) (i32.le_u (global.get $calls) (i32.const 1))
            (then (i64.const 0x400000000a))
            (else (if (result i64) (i32.eq (global.get $calls) (i32.const 2))
                (then (i64.const 0x8000000014))
                (else (unreachable)))))
        "#,
    );

    assert_eq!(
        remapper.input(press("A"), NOW),
        vec![press("X")],
        "the module answers its second call by pressing X"
    );
    assert_eq!(
        remapper.input(release("A"), NOW),
        vec![release("X")],
        "the module traps on its third, and X is not left held"
    );
    assert_eq!(remapper.take_problems().len(), 1);
}
