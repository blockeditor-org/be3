use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};
use beui_view::components::overlay::OverlaysWithin;

#[component]
fn Inner(inner: StoredCell<Option<OverlaysWithin>>, open: Signal<bool>) -> NodeId {
    let within = OverlaysWithin::new();
    inner.set(Some(within));
    provide_context(within);
    view! {
        <Overlay anchor=OverlayAnchor::Point(pos2(10.0, 10.0)) placement=Placement::At open={open}>
            <Frame width=40.0 height=30.0 />
        </Overlay>
    }
}

#[test]
fn a_nested_overlays_within_counts_toward_the_one_around_it() {
    let _root = crate::reactive::RootScope::new();
    let outer = OverlaysWithin::new();
    let inner = StoredCell::new(None);
    let open = create_signal(false);
    let mounted = create_signal(true);
    let document = build_node(move || {
        provide_context(outer);
        view! {
            <List spacing=0.0>
                <Show value={mounted}>
                    <Inner inner open />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let inner = inner.get().expect("the inner one is made");
    let mut set = |signal: Signal<bool>, value: bool| {
        with_installed(harness.document_mut(), |_| signal.set(value));
        harness.frame(Vec::new());
    };
    assert!(!outer.any());

    set(open, true);
    assert!(inner.any());
    assert!(
        outer.any(),
        "the outer one hears an overlay opened inside the inner one"
    );

    set(open, false);
    assert!(!outer.any(), "and hears it close");

    set(open, true);
    set(mounted, false);
    assert!(
        !outer.any(),
        "an inner one taken away with an overlay open stops counting"
    );
}
