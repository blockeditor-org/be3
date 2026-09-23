use super::*;

#[test]
fn dragging_a_clip_past_the_next_one_reorders_the_base_track() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Video::new());
    let clips = [Uuid::new_v4(), Uuid::new_v4()];
    for (index, clip) in clips.iter().enumerate() {
        let source = client.create_block(Counter::default());
        block.operate(VideoOperation::InsertClip {
            clip: VideoClip {
                id: *clip,
                block_id: BlockRef::Direct(source.id()),
                length: 30,
                attachment: None,
                effects: Vec::new(),
            },
            index,
        });
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::<VideoApp>::new(Editor::new(host, client, block.id()));
    editor.run();
    editor.run();
    editor.run();

    let first = editor.rect_of(&format!("video.clip.{}", clips[0]));
    let second = editor.rect_of(&format!("video.clip.{}", clips[1]));
    editor.drag(
        first.center(),
        Pos2::new(second.right() - 3.0, second.center().y),
    );
    editor.run();
    editor.run();

    let order = block
        .read()
        .unwrap()
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
