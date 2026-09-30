use super::*;
use crate::reactive::{build, clone, create_memo, view};
use crate::unstyled::{Container, DockMode, DockState, Side, TabId, narrower_than};

const BREAKPOINT: f32 = 600.0;

#[test]
fn a_dock_stacked_by_narrowing_its_container_leaves_the_hidden_panel_unplaced() {
    let home = NodeRef::new();
    let document = build({
        let home = home.clone();
        move || {
            view! {
                <Container>
                    {move |_| {
                        let home = home.clone();
                        let narrow = narrower_than(BREAKPOINT);
                        let mode = create_memo(clone!(narrow -> move || match narrow.get() {
                            true => DockMode::Stacked,
                            false => DockMode::Tiled,
                        }));
                        let mut layout = DockState::new([TabId::new(1)]);
                        let leaf = layout.leaves(layout.main())[0];
                        layout.split(leaf, Side::Right, 0.5, vec![TabId::new(2)]);
                        layout.show(TabId::new(2));
                        let (state, set_state) = create_signal(layout);
                        view! {
                            <styled::DockArea
                                state={state}
                                mode={mode}
                                home={Some(TabId::new(1))}
                                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                                on_change={move |next: DockState| set_state.set(next)}
                                on_close={move |_: TabId| {}}
                            >
                                {move |tab: TabId| match tab.value() {
                                    1 => view! {
                                        <Frame @node_ref=&home />
                                    },
                                    _ => view! {
                                        <Frame />
                                    },
                                }}
                            </styled::DockArea>
                        }
                    }}
                </Container>
            }
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let home = home.get();
    assert!(
        harness.document().node_rect(home).is_some(),
        "the tiled dock lays out the home tab beside the other"
    );

    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().node_rect(home),
        None,
        "the stacked dock shows the other tab, so the home tab's panel has no place on screen"
    );
}
