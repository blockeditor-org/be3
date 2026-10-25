use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{
    TAG_DEFINITIONS, TAGGED, Tag, TagColor, TagContent, Tags, TagsContent, WORKSPACE_TYPE,
};

use super::*;
use crate::tags::TagsApp;

#[test]
fn a_workspaces_tags_are_added_named_and_list_the_blocks_they_are_on() {
    let (workspace, tags, urgent, notes) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let host = EditorHost::default();
    host.set_block_type(TagsContent::CONTENT_TYPE);
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let mut editor = BeuiTest::<TagsApp>::new(OwnedEditor::new(host, tags));
    let store = editor.store();
    let mut team = BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root);
    team.set_name("Team");
    team.metadata.insert(&TAG_DEFINITIONS, &BlockRef(tags));
    team.references = vec![tags, notes];
    store.add_block(team);
    store.add_block(BlockInfo::new(
        tags,
        TagsContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    ));
    let mut tag = BlockInfo::new(urgent, TagContent::CONTENT_TYPE, BlockParent::Block(tags));
    tag.set_name("Urgent");
    store.add_block(tag);
    let mut tagged = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(workspace));
    tagged.set_name("Launch notes");
    tagged.metadata.insert(&TAGGED, &vec![BlockRef(urgent)]);
    tagged.references = vec![urgent];
    store.add_block(tagged);
    let mut listed = TagsContent::default();
    listed.apply(&Tags::add(urgent));
    editor.hold(None, listed);
    editor.hold(
        Some(urgent),
        TagContent::new(&Tag {
            name: "Urgent".to_owned(),
            color: TagColor::Red,
        }),
    );
    editor.run();

    editor.click("tags.new-name");
    editor.run();
    editor.type_text("Later");
    editor.run();
    editor.click("tags.add");
    editor.run();

    let held = editor.content::<TagsContent>(None).root().tags();
    assert_eq!(held.len(), 2, "adding a tag lists it beside the other");
    let added = held
        .into_iter()
        .find(|tag| *tag != urgent)
        .expect("the new tag is a block of its own");
    let store = editor.store();
    assert_eq!(
        store.block(added).and_then(|info| info.parent),
        Some(BlockParent::Block(tags)),
        "a tag lives under the tags it is one of"
    );
    editor.snapshot("a_workspaces_tags_are_added_named_and_list_the_blocks_they_are_on");
    assert!(
        editor.shown(&format!("tags.block.{notes}")),
        "a tag lists the blocks it is on"
    );
}
