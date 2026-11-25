//! What tests drive and read documents with: the headless platform's harness, and the
//! fixtures beui's own tests and the space layer's share.

pub use beui_platform_headless::testing::*;

use beui::dock;
use beui::dock_host;
use beui::primitives::{Canvas, CanvasItem, Frame};
use beui::reactive::*;
use beui::{Color32, Modifiers, Rect, pos2, vec2};
use beui::unstyled;

use super::{Document, build};

/// A document showing `view` in a black root, with the inspector and the on-screen keyboard.
pub fn build_node(view: impl FnOnce() -> NodeId) -> Document {
    build(|| {
        let content = view();
        view! {
            <beui::primitives::Root clear_color={Color32::BLACK}>
                {content}
            </beui::primitives::Root>
        }
    })
}

/// A dock of two panes split side by side ("Tab 1" and "Tab 2") and two floating windows
/// ("Tab 3" and "Tab 4"), and the dock host's node.
pub fn windowed_dock(drag_modifier: Option<Modifiers>) -> (Document, NodeId) {
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = dock::DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost drag_modifier @node_ref=&dock>
                <dock::Docking layout>
                    <dock::DockSplit id="split">
                        <dock::DockPane id="left">
                            <dock::DockTab id=1u64 title="Tab 1">
                                <Frame />
                            </dock::DockTab>
                        </dock::DockPane>
                        <dock::DockPane id="right">
                            <dock::DockTab id=2u64 title="Tab 2">
                                <Frame />
                            </dock::DockTab>
                        </dock::DockPane>
                    </dock::DockSplit>
                    <dock::DockWindow
                        id="first"
                        rect={Rect::from_min_size(pos2(60.0, 60.0), vec2(360.0, 240.0))}
                    >
                        <dock::DockPane id="third">
                            <dock::DockTab id=3u64 title="Tab 3">
                                <Frame />
                            </dock::DockTab>
                        </dock::DockPane>
                    </dock::DockWindow>
                    <dock::DockWindow
                        id="second"
                        rect={Rect::from_min_size(pos2(520.0, 200.0), vec2(360.0, 240.0))}
                    >
                        <dock::DockPane id="fourth">
                            <dock::DockTab id=4u64 title="Tab 4">
                                <Frame />
                            </dock::DockTab>
                        </dock::DockPane>
                    </dock::DockWindow>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    (document, dock.get())
}

/// The shown title of a dock's tab.
pub fn dock_tab(document: &Document, dock: NodeId, title: &str) -> NodeId {
    find_text(document, dock, title, Match::Exact)
        .unwrap_or_else(|| panic!("the dock shows {title}"))
}

/// A `PanZoom` (`stage`) over a canvas holding one 100 by 50 item (`item`) at its origin.
#[component]
pub fn PanZoomStage(
    #[prop(default = unstyled::PanZoomView::IDENTITY)] view: Binding<unstyled::PanZoomView>,
) -> NodeId {
    view! {
        <unstyled::PanZoom @test_id="stage" view>|handle: unstyled::PanZoomHandle| {
            let unstyled::PanZoomHandle { view, .. } = handle;
            view! {
                <Canvas view>
                    <CanvasItem @test_id="item" x=0.0 y=0.0 width=100.0 height=50.0 />
                </Canvas>
            }
        }</unstyled::PanZoom>
    }
}
