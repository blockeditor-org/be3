use super::*;
use beui::styled::{self, ButtonVariant, TextColor, TextSize, Weight};
use beui_inspector::{Inspector, InspectorTab, InspectorTools, scale};

fn inspector(test: &DocumentTest) -> &Inspector {
    test.document()
        .tools::<InspectorTools>()
        .unwrap()
        .inspector
        .as_ref()
        .unwrap()
}

fn expand_internals(test: &mut DocumentTest, component: &str, test_id: &str) {
    let inspector = inspector(test);
    let component = inspector
        .entries
        .iter()
        .find(|entry| {
            entry.kind == component
                && test
                    .document()
                    .node_test_ids(entry.key.node())
                    .iter()
                    .any(|id| id == test_id)
        })
        .expect("the component should be visible in the inspector");
    let internals = inspector
        .entries
        .iter()
        .find(|entry| {
            entry.kind == "internals"
                && entry.key.node() == component.key.node()
                && entry.depth == component.depth + 1
        })
        .expect("the component should have an internals row");
    assert!(!internals.expanded);
    let chevron = format!("{}.chevron", internals.key.test_id());
    let node = inspector.find(&chevron);
    let center = inspector
        .document
        .node_rect(node)
        .unwrap()
        .scaled(scale(test.context()))
        .center();
    test.click(center);
    test.frame(Vec::new());
}

#[test]
fn the_inspector_paints_the_component_shadow_tree() {
    let document = build(|| {
        view! {
            <Root>
                <styled::Frame padding=styled::Space::Xl3>
                    <styled::List spacing=styled::Space::Xl>
                        <styled::Text
                            @test_id="example.heading"
                            size=TextSize::Xl
                            weight=Weight::Bold
                        >
                            "Release checklist"
                        </styled::Text>
                        <styled::Card @test_id="example.card">
                            <styled::List spacing=styled::Space::Lg>
                                <styled::Text weight=Weight::Bold>"Ready for review"</styled::Text>
                                <styled::Text color=TextColor::Muted>
                                    "Check the build, then publish the release."
                                </styled::Text>
                                <styled::Checkbox label="All tests pass" checked=true />
                                <styled::Button
                                    @test_id="example.publish"
                                    label="Publish release"
                                    variant=ButtonVariant::Primary
                                />
                            </styled::List>
                        </styled::Card>
                        <styled::Text color=TextColor::Muted>
                            "Draft changes are saved automatically."
                        </styled::Text>
                    </styled::List>
                </styled::Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1200.0, 1100.0));
    test.frame(vec![
        Input::Modifiers(beui::Modifiers {
            ctrl: true,
            shift: true,
            ..beui::Modifiers::NONE
        }),
        Input::Key(beui::Keystroke {
            key: Some(beui::Key::I),
            pressed: true,
            ..Default::default()
        }),
        Input::Modifiers(beui::Modifiers::NONE),
    ]);
    test.frame(Vec::new());
    let panel = inspector(&test);
    let tab = panel.tab_node(InspectorTab::Components);
    let center = panel
        .document
        .node_rect(tab)
        .unwrap()
        .scaled(scale(test.context()))
        .center();
    test.click(center);
    test.frame(Vec::new());

    expand_internals(&mut test, "styled::Text", "example.heading");
    expand_internals(&mut test, "styled::Card", "example.card");
    expand_internals(&mut test, "styled::Frame", "example.card");
    expand_internals(&mut test, "styled::Button", "example.publish");
    expand_internals(&mut test, "unstyled::Button", "example.publish");

    let entries = &inspector(&test).entries;
    for label in [
        "primitives::Frame",
        "primitives::Text",
        "primitives::Interactive",
        "styled::ButtonFace",
        "\"Release checklist\"",
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind == label),
            "the painting should show {label}"
        );
    }
    test.snapshot("inspector_component_shadow_tree");
}
