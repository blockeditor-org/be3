use super::*;
use beui::reactive::view;
use beui::styled::{ColorRole, Frame, List, Size, Space, Text};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn a_show_with_a_fallback_swaps_the_one_child_a_frame_holds() {
    let held = Rc::new(Cell::new(None));
    let given = held.clone();
    let document = build(move || {
        let editing = create_signal(false);
        given.set(Some(editing));
        view! {
            <Root>
                <List spacing=Space::None>
                    <Frame @test_id="holder" fill=ColorRole::Surface padding=Space::Md>
                        <Show
                            value={editing}
                            fallback={|| view! {
                                <Frame @test_id="reading" height=Size::Sm>
                                    <Text>"Reading"</Text>
                                </Frame>
                            }}
                        >||
                            <Frame @test_id="editing" height=Size::Lg>
                                <Text>"Editing, in place of what was read"</Text>
                            </Frame>
                        </Show>
                    </Frame>
                </List>
            </Root>
        }
    });
    let editing = held.get().expect("the view was built");
    let mut test = DocumentTest::new(document, Vec2::new(320.0, 160.0));
    assert!(test.shown("reading") && !test.shown("editing"));
    let reading = test.rect_of("holder");

    editing.set(true);
    test.frame(Vec::new());
    assert!(test.shown("editing") && !test.shown("reading"));
    assert!(
        test.rect_of("holder").height() > reading.height(),
        "the frame lays out the child it holds now"
    );
    test.snapshot("a_show_with_a_fallback_swaps_the_one_child_a_frame_holds");

    editing.set(false);
    test.frame(Vec::new());
    assert!(test.shown("reading") && !test.shown("editing"));
    assert_eq!(test.rect_of("holder"), reading);
}
