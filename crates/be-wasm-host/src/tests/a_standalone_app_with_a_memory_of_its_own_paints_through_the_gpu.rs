use super::*;

const STANDALONE: &str = include_str!("standalone.wat");

#[test]
fn a_standalone_app_with_a_memory_of_its_own_paints_through_the_gpu() {
    let mut app = host().load_bytes(STANDALONE.as_bytes()).unwrap();

    app.start().unwrap();
    app.step().unwrap();

    assert_eq!(app.take_outbound(), vec![vec![1]], "it painted once");
    assert_eq!(app.take_presented(), vec![7], "and presented its surface");
    let (texture, _) = app.surface(7).expect("the surface has a target");
    assert_eq!((texture.width(), texture.height()), (4, 4));
    app.step().unwrap();
    assert_eq!(app.take_outbound(), vec![vec![2]]);
}
