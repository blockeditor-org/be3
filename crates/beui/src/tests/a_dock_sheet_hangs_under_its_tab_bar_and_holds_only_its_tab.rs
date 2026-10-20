use super::*;
use crate::unstyled::DockDialogPresentation;

const SHEET: u64 = 10;
const SHEET_SIZE: Vec2 = Vec2::new(520.0, 240.0);

struct SheetDock {
    document: Document,
    layout: unstyled::DockingLayout<u64>,
    pressed: Signal<Vec<u64>>,
    built: Signal<u32>,
    attended: ReadSignal<Vec<u64>>,
    open: Signal<bool>,
}

#[component]
fn SheetBody(built: Signal<u32>, attended: Signal<Vec<u64>>) -> NodeId {
    built.update(|built| *built += 1);
    if let Some(tab) = unstyled::use_dock_tab() {
        let called = crate::reactive::create_memo(move || tab.attention());
        crate::reactive::create_effect(move || {
            if called.get().is_some() {
                attended.update(|attended| attended.push(SHEET));
            }
        });
    }
    view! {
        <Interactive focusable=true @test_id="sheet.field">
            <Frame width=120.0 height=40.0 />
        </Interactive>
    }
}

#[component]
fn PressPanel(owner: u64, pressed: Signal<Vec<u64>>) -> NodeId {
    view! {
        <Interactive
            focusable=true
            @test_id={format!("content.{owner}")}
            on_press={move |_: crate::input::PointerPress| pressed.update(|pressed| pressed.push(owner))}
        >
            <Frame />
        </Interactive>
    }
}

fn sheet_dock() -> SheetDock {
    let pressed = create_signal(Vec::new());
    let built = create_signal(0);
    let attended = create_signal(Vec::new());
    let open = create_signal(true);
    let layout = unstyled::DockingLayout::new();
    let document = build(move || {
        let sheets = crate::reactive::create_memo(move || match open.get() {
            true => vec![SHEET],
            false => Vec::new(),
        });
        view! {
            <styled::Docking layout>
                <unstyled::DockSplit id="root" fraction=0.55>
                    <unstyled::DockPane id="side">
                        <unstyled::DockTab id=3_u64 title="Side">
                            <PressPanel owner=3 pressed />
                        </unstyled::DockTab>
                    </unstyled::DockPane>
                    <unstyled::DockPane id="tabs">
                        <ForEach keys={vec![1_u64, 2]}>|id: u64|
                            <unstyled::DockTab id title={format!("Tab {id}")}>
                                <PressPanel owner=id pressed />
                            </unstyled::DockTab>
                        </ForEach>
                    </unstyled::DockPane>
                </unstyled::DockSplit>
                <ForEach keys={sheets}>|id: u64|
                    <unstyled::DockDialog
                        id
                        parent=1_u64
                        title="Sheet"
                        size=SHEET_SIZE
                        presentation=DockDialogPresentation::Sheet
                        on_close={move || open.set(false)}
                    >
                        <SheetBody built attended />
                    </unstyled::DockDialog>
                </ForEach>
            </styled::Docking>
        }
    });
    SheetDock {
        document,
        layout,
        pressed,
        built,
        attended: attended.read_only(),
        open,
    }
}

#[test]
fn a_dock_sheet_hangs_under_its_tab_bar_and_holds_only_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let dock = sheet_dock();
    let (layout, pressed, built, attended, open) = (
        dock.layout,
        dock.pressed,
        dock.built,
        dock.attended,
        dock.open,
    );
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.1"));
    let sheet = harness.rect(harness.find("dock.dialog.10"));
    assert!(
        (sheet.top() - panel.top()).abs() < 1.0,
        "the sheet hangs from the top of its tab's panel: {sheet:?} under {panel:?}"
    );
    assert!(
        (sheet.center().x - panel.center().x).abs() < 1.0,
        "the sheet is centred across its tab's panel: {sheet:?} over {panel:?}"
    );
    assert!(
        panel.contains_rect(sheet) && sheet.width() < SHEET_SIZE.x,
        "a pane narrower than the sheet shrinks it to fit: {sheet:?} in {panel:?}"
    );
    assert_eq!(
        sheet.height(),
        SHEET_SIZE.y,
        "the sheet keeps the height it asked for where there is room"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_some(),
        "its tab shows that it waits on the sheet"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.2.waiting")
            .is_none(),
        "no other tab does"
    );
    let field = harness.find("sheet.field");
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "the sheet takes the focus"
    );

    harness.click(harness.center(field));
    harness.frame(Vec::new());
    assert!(
        attended.with_untracked(Vec::is_empty),
        "a press inside the sheet calls no attention"
    );

    let below = pos2(panel.center().x, panel.bottom() - 12.0);
    harness.click(below);
    harness.frame(Vec::new());
    assert!(
        pressed.with_untracked(Vec::is_empty),
        "the dimmed tab hears no press"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![SHEET],
        "a press on it calls the sheet's attention"
    );

    let side = harness.find("content.3");
    harness.click(harness.center(side));
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        vec![3],
        "the other pane still takes presses"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(side),
        "and the focus"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![SHEET],
        "without calling the sheet's attention"
    );

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "the sheet hides while another tab is shown in its place"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_some(),
        "its tab still shows that it waits"
    );
    let other = harness.find("content.2");
    harness.click(harness.center(other));
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 2],
        "the tab shown in its place takes presses"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("dock.dialog.10")),
        sheet,
        "the sheet comes back where it was"
    );
    assert_eq!(
        harness.find("sheet.field"),
        field,
        "with the content it had"
    );
    assert_eq!(built.get_untracked(), 1, "which was never built again");
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "and the focus goes back into it"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(!open.get_untracked(), "Escape asks the sheet to close");
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "and it closes"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_none(),
        "taking the dot with it"
    );
    harness.click(below);
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 2, 1],
        "the tab takes presses again"
    );
}
