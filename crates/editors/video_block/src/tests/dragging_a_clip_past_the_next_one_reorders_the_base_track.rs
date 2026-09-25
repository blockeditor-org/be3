use super::*;

#[test]
fn dragging_a_clip_past_the_next_one_reorders_the_base_track() {
    let mut fixture = Fixture::new();
    let clips = [fixture.insert(0), fixture.insert(1)];
    fixture.settle();

    let first = fixture.editor.rect_of(&format!("video.clip.{}", clips[0]));
    let second = fixture.editor.rect_of(&format!("video.clip.{}", clips[1]));
    fixture.editor.drag(
        first.center(),
        Pos2::new(second.right() - 3.0, second.center().y),
    );
    fixture.editor.run();
    fixture.editor.run();

    let order = fixture
        .video()
        .clips()
        .iter()
        .map(|clip| clip.id)
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [clips[1], clips[0]],
        "the dragged clip lands after the other one"
    );
}
