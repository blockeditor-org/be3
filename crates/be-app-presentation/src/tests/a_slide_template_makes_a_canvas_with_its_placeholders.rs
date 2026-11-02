use super::*;
use be_sdk_beui::be_block::BlockContent;
use be_sdk_beui::{BeuiRoot, Creation, GraphCommand};
use be_type_infinite_canvas::CanvasContent;
use be_type_infinite_canvas::CanvasShape;

#[test]
fn a_slide_template_makes_a_canvas_with_its_placeholders() {
    let host = EditorHost::default();
    let creation = Creation::for_template(host.clone(), "title-slide");

    let id = PresentationRoot::create_block(&creation).unwrap();

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
    let entities = CanvasContent::decode(&content).unwrap().root().entities();
    let slide = entities
        .first()
        .filter(|entity| entity.is_frame())
        .expect("the slide is a frame beneath its placeholders");
    assert_eq!(
        (slide.transform.size.x, slide.transform.size.y),
        (960.0, 540.0)
    );
    let texts: Vec<_> = entities
        .into_iter()
        .filter(|entity| entity.shape == CanvasShape::Text)
        .collect();
    let placeholders: Vec<_> = texts.iter().map(|text| text.placeholder.clone()).collect();
    assert_eq!(placeholders, ["Title", "Subtitle"]);
    assert!(
        texts.iter().all(|text| text.text_style.wrap),
        "placeholders are areas that keep their box, not texts that fit their content"
    );

    let unknown = Creation::for_template(host, "no-such-slide");
    assert!(PresentationRoot::create_block(&unknown).is_err());
}
