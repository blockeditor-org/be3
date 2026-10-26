use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_press_and_release_in_one_frame_on_a_submenu_item_selects_it_once() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let selected = create_signal(Vec::new());
    let (document, _) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Profiles">
                <unstyled::MenuItem label="Profile 1" />
                <unstyled::MenuItem label="Profile 2" />
            </unstyled::MenuItem>
        };
        [view! {
            <MenuButton
                @node_ref=&button
                label="More"
                items
                on_select={move |path: Vec<usize>| {
                    selected.update(|selected| selected.push(path));
                }}
            />
        }]
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(button));
    harness.frame(Vec::new());
    let find = |harness: &Harness, label: &str| {
        let document = harness.document();
        document
            .layers()
            .iter()
            .filter_map(|layer| layer.overlay)
            .find_map(|overlay| text_within(document, overlay.id(), label))
            .unwrap_or_else(|| panic!("the menu shows {label}"))
    };
    let profiles = find(&harness, "Profiles");
    harness.frame(vec![Input::PointerMoved(harness.center(profiles))]);
    harness.frame(Vec::new());
    let second = harness.center(find(&harness, "Profile 2"));
    harness.frame(vec![Input::PointerMoved(second)]);
    harness.frame(vec![
        Input::PointerButton {
            pos: second,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos: second,
            button: PointerButton::Primary,
            pressed: false,
        },
    ]);
    harness.frame(Vec::new());

    assert_eq!(selected.get_untracked().as_slice(), &[vec![0usize, 1usize]]);
}
