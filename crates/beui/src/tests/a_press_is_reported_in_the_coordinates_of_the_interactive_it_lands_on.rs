use super::*;
use crate::input::PointerPress;
use crate::reactive::{Frame, Interactive, NodeRef, build, view};

#[test]
fn a_press_is_reported_in_the_coordinates_of_the_interactive_it_lands_on() {
    let _root = crate::reactive::RootScope::new();
    let (heard, set_heard) = create_signal(Vec::<PointerPress>::new());
    let target = NodeRef::new();
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Frame height=70.0 />
                    <Frame padding_horizontal=30.0>
                        <Interactive
                            @node_ref={&target}
                            on_click_at={move |press: PointerPress| set_heard.update(|heard| heard.push(press))}
                        >
                            <Frame height=40.0 />
                        </Interactive>
                    </Frame>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(target.get());
    let at = rect.min + vec2(5.0, 10.0);
    harness.click(at);
    let heard = heard.get_untracked();
    assert_eq!(heard.len(), 1, "the click lands on the interactive");
    assert_eq!(heard[0].pos, at, "pos stays in the document's coordinates");
    assert_eq!(
        heard[0].local,
        pos2(5.0, 10.0),
        "local is from the interactive's corner"
    );
    assert_eq!(heard[0].size, rect.size(), "size is the interactive's own");
}
