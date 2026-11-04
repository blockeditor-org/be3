use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::input::{BackEdge, BackGesture};

#[test]
fn a_back_handler_goes_back_while_enabled_without_moving_its_content() {
    let _root = crate::reactive::RootScope::new();
    let enabled = create_signal(false);
    let backs = create_signal(0);
    let page = NodeRef::new();
    let document = build_node(move || {
        view! {
            <BackTo
                enabled={enabled}
                on_gesture={move |gesture: BackGesture| if gesture == BackGesture::Invoked { backs.update(|backs| *backs += 1) }}
            >
                <Text color=Color32::WHITE @node_ref={&page}>"Page"</Text>
            </BackTo>
        }
    });

    let mut harness = Harness::new(document);
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 0);

    enabled.set(true);
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
    let resting = harness.rect(page.get());

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    assert_eq!(harness.rect(page.get()), resting);
    assert_eq!(backs.get_untracked(), 0);

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 1);

    harness.key(Key::BrowserBack, Modifiers::NONE);
    assert_eq!(backs.get_untracked(), 2);
}
