use super::*;
use beui::styled::{self, ButtonVariant, TextColor, TextSize, Weight};
use beui_inspector::{Inspector, InspectorTools, scale};

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
    test.click_at(center);
    test.frame(Vec::new());
}

#[test]
fn the_inspector_paints_the_component_shadow_tree() {
    let document = build(|| {
        view! {
            <styled::Frame padding={Sides::all(24.0)}>
                <styled::List spacing=16.0>
                    <styled::Text
                        @test_id="example.heading"
                        string="Release checklist"
                        size=TextSize::Xl
                        weight=Weight::Bold
                    />
                    <styled::Card @test_id="example.card">
                        <styled::List spacing=12.0>
                            <styled::Text string="Ready for review" weight=Weight::Bold />
                            <styled::Text
                                string="Check the build, then publish the release."
                                color=TextColor::Muted
                            />
                            <styled::Checkbox label="All tests pass" checked=true />
                            <styled::Button
                                @test_id="example.publish"
                                label="Publish release"
                                variant=ButtonVariant::Primary
                            />
                        </styled::List>
                    </styled::Card>
                    <styled::Text
                        string="Draft changes are saved automatically."
                        color=TextColor::Muted
                    />
                </styled::List>
            </styled::Frame>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1200.0, 1100.0));
    test.frame(vec![beui::Event::key(beui::KeyPress {
        key: beui::Key::I,
        pressed: true,
        modifiers: beui::Modifiers {
            ctrl: true,
            shift: true,
            ..beui::Modifiers::NONE
        },
        repeat: false,
    })]);
    test.frame(Vec::new());
    let panel = inspector(&test);
    let tab = panel.option_node("inspector.tabs", 1);
    let center = panel
        .document
        .node_rect(tab)
        .unwrap()
        .scaled(scale(test.context()))
        .center();
    test.click_at(center);
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
