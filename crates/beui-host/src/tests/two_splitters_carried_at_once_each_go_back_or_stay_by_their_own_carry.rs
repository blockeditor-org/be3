use super::*;

#[test]
fn two_splitters_carried_at_once_each_go_back_or_stay_by_their_own_carry() {
    let _root = crate::reactive::RootScope::new();
    let presses = create_signal(0);
    let document = build_node(move || {
        let layout = dock::DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout>
                    <dock::DockSplit id="outer">
                        <dock::DockPane id="left">
                            <CountingTab id=1 presses={presses} />
                        </dock::DockPane>
                        <dock::DockSplit id="inner">
                            <dock::DockPane id="middle">
                                <CountingTab id=2 presses={presses} />
                            </dock::DockPane>
                            <dock::DockPane id="right">
                                <CountingTab id=3 presses={presses} />
                            </dock::DockPane>
                        </dock::DockSplit>
                    </dock::DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let width = |harness: &Harness, id: &str| harness.rect(harness.find(id)).width();
    let (left, middle) = (
        harness.rect(harness.find("content.1")),
        harness.rect(harness.find("content.2")),
    );
    let first = pos2((left.right() + middle.left()) / 2.0, middle.center().y);

    // The mouse carries the first splitter, a finger the second.
    harness.down(first, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(first + vec2(30.0, 0.0))]);
    harness.frame(Vec::new());
    // The inner split shares out what the outer one leaves it.
    let (middle, right) = (
        harness.rect(harness.find("content.2")),
        harness.rect(harness.find("content.3")),
    );
    let second = pos2((middle.right() + right.left()) / 2.0, middle.center().y);
    harness.touch(TouchPhase::Start, second);
    harness.touch(TouchPhase::Move, second + vec2(20.0, 0.0));
    harness.touch(TouchPhase::Move, second + vec2(60.0, 0.0));
    harness.frame(Vec::new());
    assert!(
        (width(&harness, "content.1") - left.width() - 30.0).abs() < 1.0,
        "the mouse moves the first splitter"
    );
    assert!(
        (width(&harness, "content.3") - right.width() + 60.0).abs() < 1.0,
        "while the finger moves the second"
    );

    // The finger's press is cancelled: only its splitter goes back.
    harness.touch(TouchPhase::Cancel, second + vec2(60.0, 0.0));
    harness.frame(Vec::new());
    harness.settle();
    assert_eq!(
        width(&harness, "content.3"),
        right.width(),
        "the second splitter goes back to where it was pressed"
    );
    assert!(
        (width(&harness, "content.1") - left.width() - 30.0).abs() < 1.0,
        "and the first stays with the mouse"
    );
    harness.frame(vec![Input::PointerMoved(first + vec2(40.0, 0.0))]);
    harness.up(first + vec2(40.0, 0.0), PointerButton::Primary);
    harness.settle();
    let kept = width(&harness, "content.1");
    assert!(
        (kept - left.width() - 40.0).abs() < 1.0,
        "the first is dropped where the mouse lets go"
    );

    // Carried again, Escape puts it back: a kept drop does not outlast its own carry.
    let first = first + vec2(40.0, 0.0);
    harness.down(first, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(first + vec2(30.0, 0.0))]);
    harness.key(Key::Escape, Modifiers::NONE);
    harness.up(first + vec2(30.0, 0.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(
        width(&harness, "content.1"),
        kept,
        "Escape puts the first back where its second carry began"
    );
}
