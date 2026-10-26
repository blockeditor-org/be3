use super::*;

#[test]
fn the_pointer_cannot_leave_the_outputs() {
    let outputs = arrange(&[((100, 100), 1.0), ((50, 50), 1.0)]);

    assert_eq!(
        moved(pos2(90.0, 70.0), vec2(30.0, 0.0), &outputs),
        pos2(120.0, 70.0),
        "the pointer crosses onto the next output"
    );
    assert_eq!(
        moved(pos2(120.0, 70.0), vec2(0.0, -60.0), &outputs),
        pos2(120.0, 50.0),
        "above the shorter output, the pointer stops at its edge"
    );
    assert_eq!(
        moved(pos2(10.0, 10.0), vec2(-40.0, -40.0), &outputs),
        pos2(0.0, 0.0)
    );
}
