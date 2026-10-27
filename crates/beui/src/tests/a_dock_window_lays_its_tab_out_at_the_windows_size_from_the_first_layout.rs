use super::*;
use crate::reactive::*;

#[test]
fn a_dock_window_lays_its_tab_out_at_the_windows_size_from_the_first_layout() {
    let _root = crate::reactive::RootScope::new();
    let layout = dock::unstyled::DockingLayout::<u64>::new();
    let widths = create_signal(Vec::<f32>::new());
    let document = build_node(move || {
        let content = view! {
            <Frame @test_id="program" />
        };
        let measured = measure(content);
        create_effect(move || {
            if let Some(measured) = measured.get() {
                widths.update(|widths| widths.push(measured.size.x));
            }
        });
        view! {
            <dock::styled::Docking layout>
                <dock::unstyled::DockPane
                    id="desktop"
                    empty={|| view! {
                        <Frame />
                    }}
                />
                <dock::unstyled::DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(60.0, 60.0), vec2(360.0, 240.0))}
                >
                    <dock::unstyled::DockPane id="window">
                        <dock::unstyled::DockTab id={1u64} title="Program">{content}</dock::unstyled::DockTab>
                    </dock::unstyled::DockPane>
                </dock::unstyled::DockWindow>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let widths = widths.get_untracked();
    assert_eq!(
        widths.len(),
        1,
        "the window's tab is first laid out at the size it keeps, not at a guess corrected \
         a layout later: {widths:?}"
    );
    assert!(widths[0] > 0.0);
}
