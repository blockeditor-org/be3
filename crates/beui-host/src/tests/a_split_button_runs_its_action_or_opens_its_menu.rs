use accesskit::Role;

use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::MenuItem;

#[test]
fn a_split_button_runs_its_action_or_opens_its_menu() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let picked = create_signal(Vec::new());
    let (document, [split]) = toolbar_of(|| {
        [view! {
            <SplitButton
                label="Run"
                variant=ButtonVariant::Primary
                menu_label="More ways to run it"
                items={view! {
                    <MenuItem label="Run with fresh data" />
                    <MenuItem label="Install its launcher" />
                }}
                on_click={move || clicks.update(|clicks| *clicks += 1)}
                on_select={move |path: Vec<usize>| picked.update(|picked| picked.push(path))}
            />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let buttons: Vec<Option<String>> = harness
        .accessible()
        .iter()
        .filter(|node| node.role() == Role::Button)
        .map(|node| node.label().map(str::to_owned))
        .collect();
    assert_eq!(
        buttons,
        [
            Some("Run".to_owned()),
            Some("More ways to run it".to_owned())
        ]
    );

    let rect = harness.rect(split);
    let first = harness
        .document()
        .focusables_within(split)
        .first()
        .copied()
        .expect("the split button can take the focus");
    harness.click(harness.center(first));
    assert_eq!(clicks.get_untracked(), 1);
    assert!(picked.with_untracked(|picked| picked.is_empty()));

    harness.click(pos2(
        rect.left() + harness.rect(first).width() + 12.0,
        rect.center().y,
    ));
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        clicks.get_untracked(),
        1,
        "the arrow opens the menu without running the action"
    );
    assert_eq!(picked.get_untracked(), [vec![1]]);
}
