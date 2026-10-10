use super::*;
use crate::GlobalKeyPress;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::input::{BackGesture, KeyPress};
use crate::reactive::{
    Overlay, WriteSignal, build, create_signal, on_global_key, view, with_reactive_scope,
};
use crate::styled::TextInput;

#[test]
fn a_locking_overlay_stays_on_top_and_shuts_out_global_keys() {
    let (locked, set_locked) = create_signal(false);
    let (menu, set_menu) = create_signal(false);
    let globals = Rc::new(Cell::new(0));
    let dismissed = Rc::new(Cell::new(0));
    let typed = Rc::new(RefCell::new(String::new()));
    let (lock, popup) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (globals, dismissed, typed) = (globals.clone(), dismissed.clone(), typed.clone());
        let (lock, popup) = (lock.clone(), popup.clone());
        move || {
            on_global_key(move |global: GlobalKeyPress| {
                globals.set(globals.get() + 1);
                global.press.modifiers.ctrl
            });
            view! {
                <List spacing=0.0>
                    <TextInput @test_id={"outside"} value="" />
                    <Overlay
                        @node_ref=&popup
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        open={menu}
                    >
                        <Frame width=40.0 height=30.0 @test_id={"menu"} />
                    </Overlay>
                    <Overlay
                        @node_ref=&lock
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Fill
                        scrim=Color32::BLACK
                        locks=true
                        open={locked}
                        on_dismiss={move || dismissed.set(dismissed.get() + 1)}
                    >
                        <Frame width=200.0 height=40.0>
                            <TextInput
                                @test_id={"password"}
                                value=""
                                password=true
                                on_change={move |value: String| *typed.borrow_mut() = value}
                            />
                        </Frame>
                    </Overlay>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let lock = kind_of::<OverlayNode>(harness.document(), lock.get());
    let popup = kind_of::<OverlayNode>(harness.document(), popup.get());
    let outside = harness.find("outside");
    let set = |harness: &mut Harness, signal: &WriteSignal<bool>, value: bool| {
        let signal = signal.clone();
        with_reactive_scope(harness.document_mut(), move || signal.set(value));
        harness.frame(Vec::new());
    };

    harness.key(Key::K, Modifiers::CTRL);
    assert_eq!(globals.get(), 2, "global keys are heard while unlocked");
    set(&mut harness, &set_locked, true);
    assert!(harness.document().locked());

    harness.key(Key::Escape, Modifiers::NONE);
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    with_reactive_scope(harness.document_mut(), || {
        with_document(|document| document.dismiss_overlay(lock));
    });
    harness.frame(Vec::new());
    assert!(
        harness.document().is_overlay_open(lock.id()),
        "nothing the user does dismisses a lock"
    );
    assert_eq!(dismissed.get(), 0);

    let before = globals.get();
    harness.key(Key::K, Modifiers::CTRL);
    harness.key(Key::L, Modifiers::LOGO);
    assert_eq!(globals.get(), before, "no global key is heard while locked");
    harness.key(Key::VolumeUp, Modifiers::NONE);
    assert!(
        globals.get() > before,
        "media keys still reach global actions while locked"
    );
    let before = globals.get();
    harness.frame(vec![Event::InterceptedKey(KeyPress {
        key: Key::K,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    })]);
    assert_eq!(
        globals.get(),
        before,
        "a key intercepted for a plugin is not heard while locked"
    );

    set(&mut harness, &set_menu, true);
    assert!(harness.document().is_overlay_open(popup.id()));
    assert_eq!(
        harness.document().overlays_bottom_up().last(),
        Some(&lock),
        "an overlay opened while locked opens beneath the lock"
    );
    harness.click(harness.center(harness.find("password")));
    harness.document_mut().focus_focusable(outside);
    assert_ne!(
        harness.document().focused_node(),
        Some(outside),
        "nothing outside the lock can take the focus"
    );
    harness.type_text("hunter2");
    assert_eq!(
        *typed.borrow(),
        "hunter2",
        "the lock's own field takes the keys"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.type_text("!");
    assert_eq!(
        *typed.borrow(),
        "hunter2!",
        "neither Escape nor a click on the scrim takes the focus from the lock's field"
    );

    set(&mut harness, &set_locked, false);
    assert!(!harness.document().locked());
    let before = globals.get();
    harness.key(Key::K, Modifiers::CTRL);
    assert!(
        globals.get() > before,
        "global keys are heard once unlocked"
    );
}
