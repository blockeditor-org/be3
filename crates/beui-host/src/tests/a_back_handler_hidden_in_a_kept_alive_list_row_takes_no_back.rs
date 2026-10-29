use super::*;
use crate::input::BackGesture;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_back_handler_hidden_in_a_kept_alive_list_row_takes_no_back() {
    let _root = crate::reactive::RootScope::new();
    let (outer, inner) = (create_signal(0), create_signal(0));
    let shown = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <BackTo
                    on_gesture={move |gesture: BackGesture| if gesture == BackGesture::Invoked { outer.update(|backs| *backs += 1) }}
                >
                    <Frame width=40.0 height=30.0 />
                </BackTo>
                <ShowKeepAlive condition={shown}>
                    <BackTo
                        on_gesture={move |gesture: BackGesture| if gesture == BackGesture::Invoked { inner.update(|backs| *backs += 1) }}
                    >
                        <Frame width=40.0 height=30.0 />
                    </BackTo>
                </ShowKeepAlive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(
        (inner.get_untracked(), outer.get_untracked()),
        (1, 0),
        "back goes to the latest handler shown"
    );

    shown.set(false);
    harness.frame(Vec::new());
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(
        (inner.get_untracked(), outer.get_untracked()),
        (1, 1),
        "a handler in a row that is hidden but kept alive takes no back"
    );

    shown.set(true);
    harness.frame(Vec::new());
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(
        (inner.get_untracked(), outer.get_untracked()),
        (2, 1),
        "shown again, it takes back again"
    );
}
