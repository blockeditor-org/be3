use super::*;
use crate::base::overlay::OverlayNode;
use crate::native::{with_document, with_reactive_scope};
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::{Dismiss, KeyEvent, LayerLevel};
use beui::BackGesture;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn a_locking_overlay_stays_on_top_and_shuts_out_global_keys() {
    let _root = crate::reactive::RootScope::new();
    let locked = create_signal(false);
    let menu = create_signal(false);
    let globals = create_signal(0);
    let dismissed = create_signal(0);
    let typed = create_signal(String::new());
    let (lock, popup) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        on_global_key(move |event: KeyEvent| {
            globals.update(|globals| *globals += 1);
            event.press().is_some_and(|press| press.modifiers.ctrl)
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
                    level=LayerLevel::Lock
                    open={locked}
                    on_dismiss={move |_: Dismiss| dismissed.update(|dismissed| *dismissed += 1)}
                >
                    <Frame width=200.0 height=40.0>
                        <TextInput @test_id={"password"} value={typed} kind={InputKind::Password} />
                    </Frame>
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let lock = kind_of::<OverlayNode>(harness.document(), lock.get());
    let popup = kind_of::<OverlayNode>(harness.document(), popup.get());
    let outside = harness.find("outside");
    let set = |harness: &mut Harness, signal: Signal<bool>, value: bool| {
        signal.set(value);
        harness.frame(Vec::new());
    };

    harness.key(Key::K, Modifiers::CTRL);
    assert_eq!(
        globals.get_untracked(),
        4,
        "global keys are heard while unlocked, Ctrl going down and up around K"
    );
    set(&mut harness, locked, true);
    assert!(harness.document().locked());

    harness.key(Key::Escape, Modifiers::NONE);
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    with_reactive_scope(harness.document_mut(), || {
        with_document(|document| document.request_dismiss(lock, Dismiss::Back));
    });
    harness.frame(Vec::new());
    assert!(
        harness.document().is_overlay_open(lock.id()),
        "nothing the user does dismisses a lock"
    );
    assert_eq!(dismissed.get_untracked(), 0);

    let before = globals.get_untracked();
    harness.key(Key::K, Modifiers::CTRL);
    harness.key(Key::L, Modifiers::LOGO);
    assert_eq!(
        globals.get_untracked(),
        before,
        "no global key is heard while locked"
    );
    harness.key(Key::VolumeUp, Modifiers::NONE);
    assert!(
        globals.get_untracked() > before,
        "media keys still reach global actions while locked"
    );
    let before = globals.get_untracked();
    harness.frame(vec![
        Input::Modifiers(Modifiers::CTRL),
        Input::InterceptedKey {
            key: Key::K,
            pressed: true,
            repeat: false,
        },
        Input::Modifiers(Modifiers::NONE),
    ]);
    assert_eq!(
        globals.get_untracked(),
        before,
        "a key intercepted for a plugin is not heard while locked"
    );

    set(&mut harness, menu, true);
    assert!(harness.document().is_overlay_open(popup.id()));
    assert_eq!(
        harness
            .document()
            .layers()
            .last()
            .and_then(|layer| layer.overlay),
        Some(lock),
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
        typed.get_untracked(),
        "hunter2",
        "the lock's own field takes the keys"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.type_text("!");
    assert_eq!(
        typed.get_untracked(),
        "hunter2!",
        "neither Escape nor a click on the scrim takes the focus from the lock's field"
    );

    set(&mut harness, locked, false);
    assert!(!harness.document().locked());
    let before = globals.get_untracked();
    harness.key(Key::K, Modifiers::CTRL);
    assert!(
        globals.get_untracked() > before,
        "global keys are heard once unlocked"
    );
}
