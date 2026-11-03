use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::Overflow;

#[test]
fn a_text_says_whether_it_is_cut_short_whenever_that_changes() {
    let _root = crate::reactive::RootScope::new();
    let width = create_signal(40.0_f32);
    let heard = create_signal(Vec::<bool>::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame width={width} height=20.0>
                    <Text
                        overflow=Overflow::Ellipsis
                        on_truncated={move |cut: bool| heard.update(|heard| heard.push(cut))}
                    >
                        "A name too long for a narrow tile"
                    </Text>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 40.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(heard.get_untracked(), [true], "a text cut short says so");

    with_installed(harness.document_mut(), |_| width.set(390.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        heard.get_untracked(),
        [true, false],
        "a text given room for all of it says it is whole"
    );

    with_installed(harness.document_mut(), |_| width.set(395.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        heard.get_untracked(),
        [true, false],
        "a text says nothing while it stays whole"
    );
}
