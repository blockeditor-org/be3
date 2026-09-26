use super::*;
use block_editor_plugin::be_block::canvas::CanvasEntityKind;
use block_editor_plugin::be_block::{BlockContent, CanvasContent};
use block_editor_plugin::{BeuiApp, Creation, GraphCommand};

#[test]
fn a_slide_template_makes_a_canvas_with_its_placeholders() {
    let host = EditorHost::default();
    let creation = Creation::for_template(host.clone(), "title-slide");

    let id = PresentationApp::create_block(&creation).unwrap();

    let (block_type, content) = host
        .take_graph_commands()
        .into_iter()
        .find_map(|command| match command {
            GraphCommand::Create {
                id: created,
                block_type,
                content: Some(content),
                ..
            } if created == id => Some((block_type, content)),
            _ => None,
        })
        .expect("the slide was created with content");
    assert_eq!(block_type, CanvasContent::CONTENT_TYPE);
    let placeholders: Vec<_> = CanvasContent::decode(&content)
        .unwrap()
        .root()
        .entities()
        .into_iter()
        .filter_map(|entity| match entity.kind {
            CanvasEntityKind::Text { placeholder, .. } => Some(placeholder),
            _ => None,
        })
        .collect();
    assert_eq!(placeholders, ["Title", "Subtitle"]);

    let unknown = Creation::for_template(host, "no-such-slide");
    assert!(PresentationApp::create_block(&unknown).is_err());
}
