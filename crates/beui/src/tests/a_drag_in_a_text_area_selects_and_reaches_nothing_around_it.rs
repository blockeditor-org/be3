use super::*;
use crate::input::PointerPress;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_drag_in_a_text_area_selects_and_reaches_nothing_around_it() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let around = create_signal(0_usize);
    let disabled = create_signal(false);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"Hello world")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <Interactive
                on_press={move |_: PointerPress| around.update(|count| *count += 1)}
                on_drag={move |_: PointerPress| around.update(|count| *count += 1)}
            >
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    @node_ref=&area
                    state={state}
                    line_numbers=false
                    disabled
                />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(area.get());
    let (start, end) = (
        pos2(rect.left() + 2.0, rect.top() + 16.0),
        pos2(rect.right() - 2.0, rect.top() + 16.0),
    );

    harness.drag(start, end);
    harness.type_text("Bye");
    assert_eq!(
        (
            harness.document().api::<TextAreaApi>(area.get()).shown(),
            around.get_untracked()
        ),
        ("Bye".to_owned(), 0),
        "the drag selects the text, and nothing around the field sees it"
    );

    disabled.set(true);
    harness.frame(Vec::new());
    harness.drag(start, end);
    assert!(
        around.get_untracked() > 0,
        "a disabled field leaves its presses to what is around it"
    );
}
