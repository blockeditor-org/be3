use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_disabled_button_prop_tracks_a_signal_and_blocks_clicks_while_true() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let go_id = create_signal(None);
    let toggle_id = create_signal(None);

    let document = build_node(move || {
        let disabled = create_signal(true);

        let toggle = view! {
            <Button
                on_click={move || {
                    disabled.update(|disabled| *disabled = !*disabled)
                }}
            >
                <Text>"toggle"</Text>
            </Button>
        };
        toggle_id.set(Some(toggle));

        let go = view! {
            <Button disabled on_click={move || clicks.update(|clicks| *clicks += 1)}>
                <Text>"go"</Text>
            </Button>
        };
        go_id.set(Some(go));

        view! {
            <List spacing=0.0>{toggle}{go}</List>
        }
    });

    let toggle = toggle_id
        .get_untracked()
        .expect("toggle button was created");
    let go = go_id.get_untracked().expect("go button was created");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(go));
    harness.frame(Vec::new());
    assert_eq!(
        clicks.get_untracked(),
        0,
        "a disabled button must not fire on_click"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    harness.click(harness.center(go));
    harness.frame(Vec::new());
    assert_eq!(
        clicks.get_untracked(),
        1,
        "the button must become clickable once its disabled prop turns false"
    );
}
