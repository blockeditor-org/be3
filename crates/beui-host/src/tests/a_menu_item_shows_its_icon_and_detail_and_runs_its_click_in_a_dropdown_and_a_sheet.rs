use super::*;
use crate::icons::ICON_PENCIL;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::MenuItem;

fn shown(harness: &Harness, text: &str) -> Option<Pos2> {
    let root = harness.document().root().expect("the document has a root");
    text_within(harness.document(), root, text)
        .or_else(|| {
            icon_within(
                harness.document(),
                root,
                crate::icons::Glyph::from_codepoint(text)?,
            )
        })
        .map(|node| harness.center(node))
}

fn sheet_shown(harness: &Harness) -> bool {
    harness
        .document()
        .find_test_id("sheet.handle")
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_menu_item_shows_its_icon_and_detail_and_runs_its_click_in_a_dropdown_and_a_sheet() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <MenuButton
                label="Edit"
                items={view! {
                    <MenuItem
                        label="Rename"
                        glyph=ICON_PENCIL
                        detail="Ctrl+R"
                        on_click={move || clicks.update(|clicks| *clicks += 1)}
                    />
                }}
            />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.settle();

    harness.click(harness.center(button));
    harness.settle();
    assert!(!sheet_shown(&harness), "a click opens a dropdown");
    assert!(
        shown(&harness, ICON_PENCIL.as_str()).is_some(),
        "the dropdown shows the icon"
    );
    assert!(
        shown(&harness, "Ctrl+R").is_some(),
        "the dropdown shows the detail"
    );
    harness.click(shown(&harness, "Rename").expect("the dropdown lists Rename"));
    harness.settle();
    assert_eq!(clicks.get_untracked(), 1, "choosing the row runs its click");

    let at = harness.center(button);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    harness.settle();
    assert!(sheet_shown(&harness), "a tap opens a sheet");
    assert!(
        shown(&harness, ICON_PENCIL.as_str()).is_some(),
        "the sheet shows the icon"
    );
    assert!(
        shown(&harness, "Ctrl+R").is_some(),
        "the sheet shows the detail"
    );
    let rename = shown(&harness, "Rename").expect("the sheet lists Rename");
    harness.touch(TouchPhase::Start, rename);
    harness.touch(TouchPhase::End, rename);
    harness.settle();
    assert_eq!(
        clicks.get_untracked(),
        2,
        "choosing the row in the sheet runs its click"
    );
    assert!(!sheet_shown(&harness), "choosing closes the sheet");
}
