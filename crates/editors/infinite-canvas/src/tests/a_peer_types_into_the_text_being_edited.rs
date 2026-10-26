use super::*;
use block_editor_beui::be_block::Edit;
use block_editor_beui::be_block::be_model::ObjectId;
use block_editor_beui::beui::pos2;
use block_editor_beui::beui::styled::{TextAreaApi, TextAreaSurfaceApi};
use infinite_canvas_types::Entity;

fn peer_types(editor: &mut BeuiTest<CanvasApp>, entity: Uuid, at: usize, typed: &str) {
    let content = editor.content::<CanvasContent>(None);
    let rich = Canvas::rich_text(&content, entity).expect("the entity has text");
    let op = rich
        .text
        .insert(2, at, typed.as_bytes().to_vec())
        .expect("the peer has somewhere to type");
    let edit = Edit::from(Entity::TEXT.edit(ObjectId::from_uuid(entity), op));
    editor.edit::<CanvasContent>(None, &edit);
}

#[test]
fn a_peer_types_into_the_text_being_edited() {
    let mut note = entity(Uuid::from_u128(1));
    note.shape = CanvasShape::Text;
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(220.0, 40.0), 0.0);
    note.text = "Hello".into();
    let mut other = note.clone();
    other.id = Uuid::from_u128(2);
    other.transform.center = CanvasPoint::new(0.0, 80.0);
    other.text = "World".into();
    let mut editor = editor(&[note.clone(), other.clone()]);
    let field = format!("infinite-canvas.text.{}", note.id);
    let resting = editor.rect_of(&field);
    editor.double_click(pos2(resting.right() - 2.0, resting.center().y));
    editor.run();

    peer_types(&mut editor, note.id, 0, "Oh, ");
    editor.run();
    editor.type_text("!");
    editor.run();
    peer_types(&mut editor, other.id, 5, "s");
    editor.run();

    let texts: Vec<String> = entities(&editor)
        .iter()
        .map(|entity| entity.text.to_str_lossy().into_owned())
        .collect();
    assert_eq!(
        texts,
        ["Oh, Hello!", "Worlds"],
        "the peer's words are kept and typing stays at the caret"
    );
    let shown = editor
        .document()
        .find_test_id(&format!("infinite-canvas.text.{}", other.id))
        .expect("the other text is drawn");
    assert_eq!(
        editor
            .document()
            .api::<TextAreaApi>(editor.document().api::<TextAreaSurfaceApi>(shown).surface())
            .shown(),
        "Worlds",
        "text nobody here edits shows the peer's edit too"
    );
}
