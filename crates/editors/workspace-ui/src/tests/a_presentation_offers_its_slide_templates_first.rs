use block_editor_beui::{BlockLocation, CreationProgress, PickedBlock};

use super::*;

#[test]
fn a_presentation_offers_its_slide_templates_first() {
    let (mut fixture, _) = editor();
    let mut slides = catalog();
    for (template, name, icon) in [
        ("title", "Title page", "\u{e384}"),
        ("regular", "Regular page", "\u{e185}"),
        ("blank", "Blank page", "\u{e167}"),
    ] {
        slides
            .templates
            .push(block_editor_beui::TemplateDescriptor {
                editor: TEMPLATE_EDITOR.into_bytes(),
                template: template.to_owned(),
                block_type: TEMPLATE_TYPE.into_bytes(),
                name: name.to_owned(),
                icon_codepoint: icon.to_owned(),
                category: block_editor_beui::TemplateCategory::Template,
                dialog: false,
            });
    }
    fixture.test.catalog(slides);
    let mut asked = filter();
    asked.name = "Slide".to_owned();
    asked.templates = true;
    let deck = Uuid::new_v4();
    let mut fixture = picking(&fixture, asked, BlockLocation::Block(deck.into_bytes()));
    fixture.settle();

    assert!(
        fixture.says("New slide"),
        "the menu leads with the new slide"
    );
    assert!(
        fixture
            .test
            .shown(&format!("picker.menu.{TEMPLATE_EDITOR}/title")),
        "its templates come first"
    );
    assert!(
        !fixture
            .test
            .shown(&format!("picker.menu.{TEMPLATE_EDITOR}/main")),
        "the common types wait behind Other blocks"
    );
    fixture
        .test
        .snapshot("a_presentation_offers_its_slide_templates_first");

    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/title"));
    fixture.settle();
    fixture.settle();
    let created = Uuid::new_v4();
    report_creation(
        &mut fixture,
        CreationProgress::Created(created.into_bytes()),
    );
    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(Some(PickedBlock {
            block_id: created.into_bytes(),
            block_type: TEMPLATE_TYPE.into_bytes(),
            linked: false,
            placed: false,
        }))]
    );
}
