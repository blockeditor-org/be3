use super::*;
use be_sdk_beui::{HostWindowId, MenuAsk, Picture, Pos2, Subregion, SubregionContent};
use std::cell::RefCell;

thread_local! {
    static WINDOW_STATE: RefCell<ChildState> = RefCell::new(ChildState::default());
}

struct WindowRoot;

#[component]
impl BeuiRoot for WindowRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Frame width=Size::Popover height={Size::Custom(180.0)}>
                    <Subregion
                        editor={editor}
                        placed={Some(SubregionContent::Window(HostWindowId(4)))}
                        mode=ChildMode::Live
                        fullscreen=true
                        on_state={Callback::new(|state: ChildState| {
                            WINDOW_STATE.with(|held| *held.borrow_mut() = state);
                        })}
                    />
                </Frame>
            </Root>
        }
    }
}

#[test]
fn a_placed_window_hears_its_status_and_is_asked_to_close() {
    let mut test = BeuiTest::<WindowRoot>::editing(Uuid::new_v4());
    test.run();
    let placement = match test.children() {
        [placement] => placement.clone(),
        children => panic!("expected one placed child, got {}", children.len()),
    };
    assert!(placement.fullscreen, "the placement says it is shown full screen");

    let icon = Picture {
        width: 1,
        height: 1,
        rgba: vec![9, 8, 7, 255],
    };
    let ask = MenuAsk {
        asked: 1,
        at: Pos2::new(10.0, 20.0),
    };
    test.report_child(placement.child, |status| {
        status.title = Some("Terminal".to_owned());
        status.icon = Some(icon.clone());
        status.group = Some("org.example.Terminal".to_owned());
        status.raised = 2;
        status.rung = 1;
        status.fullscreen = true;
        status.notice = Some("Not responding".to_owned());
        status.menu_asked = Some(ask);
    });
    test.run();
    let state = WINDOW_STATE.with(|held| held.borrow().clone());
    assert_eq!(state.title.as_deref(), Some("Terminal"));
    assert_eq!(state.icon, Some(icon));
    assert_eq!(state.group.as_deref(), Some("org.example.Terminal"));
    assert_eq!((state.raised, state.rung), (2, 1));
    assert!(state.fullscreen);
    assert_eq!(state.notice.as_deref(), Some("Not responding"));
    assert_eq!(state.menu_asked, Some(ask));

    test.host().close_child(placement.child);
    test.run();
    assert_eq!(test.take_child_closes(), vec![placement.child]);
    assert!(test.take_child_closes().is_empty());
}
