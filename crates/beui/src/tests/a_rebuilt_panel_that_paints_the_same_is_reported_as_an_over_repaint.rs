use super::*;
use crate::reactive::{Keyed, List, ReadSignal, build, create_signal, view};

#[test]
fn a_rebuilt_panel_that_paints_the_same_is_reported_as_an_over_repaint() {
    crate::detect_over_repaint(true);
    let (key, set_key) = create_signal(0u32);
    let (color, set_color) = create_signal(Color32::WHITE);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame width=20.0 height=20.0 color={color} />
                <Keyed value={key} key={|key: u32| key}>
                    {|_: ReadSignal<u32>| view! {
                        <Frame width=300.0 height=200.0 color=Color32::from_gray(128) />
                    }}
                </Keyed>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    crate::take_over_repaints();

    with_installed(harness.document_mut(), |_| set_color.set(Color32::BLACK));
    harness.frame(Vec::new());
    assert_eq!(
        crate::take_over_repaints(),
        Vec::new(),
        "a change repainted where it changed is not reported"
    );

    with_installed(harness.document_mut(), |_| set_key.set(1));
    harness.frame(Vec::new());
    let reports = crate::take_over_repaints();
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].changed.is_empty());
    crate::detect_over_repaint(false);
}
