use super::*;
use beui::reactive::*;
use block_plugin_api::{HostTransform, LayerSpace};

thread_local! {
    static OPEN: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}

#[beui::reactive::component]
fn PopoverBody() -> beui::NodeId {
    use beui::styled::*;
    let open = create_signal(false);
    OPEN.with(|held| held.set(Some(open)));
    let control = NodeRef::new();
    view! {
        <List spacing=Space::None>
            <Button @node_ref=&control label="Settings" on_click={move || open.set(true)} />
            <Dialog
                open={open}
                title="Settings"
                anchor={DialogAnchor::Control(control)}
                on_dismiss={move || open.set(false)}
            >
                <Text>"Settings about this tab."</Text>
            </Dialog>
        </List>
    }
}

struct PopoverApp;

#[beui::reactive::component]
impl crate::BeuiApp for PopoverApp {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::styled::{DockPane, DockTab, Docking, DockingLayout};
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockPane id="tabs">
                        <DockTab id=2u64 title="Two">
                            <PopoverBody />
                        </DockTab>
                    </DockPane>
                </Docking>
            </beui::styled::Root>
        }
    }
}

#[test]
fn a_docked_editors_popover_is_an_anchored_view_of_its_tab() {
    let mut session = session::<PopoverApp>(Uuid::new_v4());
    let space = LayerSpace {
        to_host: HostTransform {
            origin: block_plugin_api::Pos2::new(100.0, 60.0),
            scale: block_plugin_api::Vec2::new(1.0, 1.0),
        },
        bounds: Rect::from_min_max(
            block_plugin_api::Pos2::ZERO,
            block_plugin_api::Pos2::new(1200.0, 800.0),
        ),
    };
    let place = |session: &mut EditorSession, shown: &[(View, u64)]| {
        let (placements, requests): (Vec<_>, Vec<_>) = shown
            .iter()
            .map(|(view, screen)| {
                let (placement, mut request) = dock_screen(*view, *screen, true);
                request.layers = Some(space.clone());
                (placement, request)
            })
            .unzip();
        session.place(&placements, &requests);
    };
    place(&mut session, &[(View::MAIN, 0)]);
    session.frame(&[(ViewId::MAIN, 0)]);
    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[2], &[2], Some(2))]);
    let shown = [(View::MAIN, 0), (View::dock(ViewId(2)), 1)];
    place(&mut session, &shown);
    session.frame(&[(ViewId::MAIN, 0), (ViewId(2), 0)]);
    session.frame(&[(ViewId::MAIN, 0), (ViewId(2), 0)]);

    let open = OPEN
        .with(std::cell::Cell::get)
        .expect("the tab body was built");
    open.set(true);
    session.frame(&[(ViewId::MAIN, 0), (ViewId(2), 0)]);
    let frames = session.frame(&[(ViewId::MAIN, 0), (ViewId(2), 0)]);

    assert!(
        frames[1]
            .anchored
            .iter()
            .any(|anchored| anchored.placement.parent == ViewId(2)),
        "the popover is an anchored view of the tab's view, drawn by its owner: {:?}",
        frames[1].anchored
    );
    assert!(
        frames[0].anchored.is_empty(),
        "nothing hangs from the docked frame, which the host does not show"
    );
}
