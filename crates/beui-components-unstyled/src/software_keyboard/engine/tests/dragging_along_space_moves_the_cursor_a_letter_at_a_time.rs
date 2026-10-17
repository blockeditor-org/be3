use super::*;

#[test]
fn dragging_along_space_moves_the_cursor_a_letter_at_a_time() {
    let mut typist = Typist::new();
    let moves: Vec<Vec2> = (1..=12).map(|step| vec2(-6.0 * step as f32, 0.0)).collect();
    typist.path(Value::Char(' '), &moves);
    let typed = typist.typed();
    assert!(
        typed.starts_with("[ArrowLeft][ArrowLeft]") && !typed.contains(' '),
        "a drag left moves the cursor left and types no space: {typed}"
    );
    let mut there: Vec<Vec2> = (1..=12).map(|step| vec2(6.0 * step as f32, 0.0)).collect();
    there.extend((1..=6).map(|step| vec2(72.0 - 6.0 * step as f32, 0.0)));
    typist.path(Value::Char(' '), &there);
    let typed = typist.typed();
    assert!(
        typed.contains("[ArrowRight]") && typed.ends_with("[ArrowLeft]"),
        "the cursor follows the finger back: {typed}"
    );
}
