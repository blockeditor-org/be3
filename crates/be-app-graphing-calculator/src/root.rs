use be_sdk_beui::{ContentProjection, Creation, Editor, Fit, Fitting, Loaded};
use beui::dock::styled::{DockBarGrip, DockBarSurface};
use beui::dock::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockSplit, DockTab, Docking,
    DockingLayout, SplitSize,
};
use beui::icons::{ICON_PANEL_LEFT_OPEN, ICON_SCAN, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use beui::reactive::*;
use beui::styled::*;
use beui::{Rect, Vec2, pos2};
use be_type_graphing_calculator::{Graph, GraphContent};
use uuid::Uuid;

mod equations;
mod plot;

use equations::Equations;
use plot::Plot;

const STAGE: &str = "graph";
const EQUATIONS: &str = "equations";
const WORLD: Vec2 = Vec2::new(600.0, 400.0);
const ZOOM_STEP: f32 = 1.25;

pub struct GraphRoot;

#[component]
impl be_sdk_beui::BeuiRoot for GraphRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <Calculator editor />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&initial_graph()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(WORLD)
    }
}

pub(crate) fn initial_graph() -> GraphContent {
    let mut content = GraphContent::default();
    for expression in ["sin(x)", "1/x"] {
        content.apply(&Graph::add(expression));
    }
    content
}

#[component]
fn Calculator(editor: Editor) -> NodeId {
    let content = editor.block_content::<GraphContent>();
    let stage = NodeRef::new();
    editor.content(&stage);
    let fitting = editor.keep_fitted(|| Some(Fit::Area(Rect::from_min_size(pos2(0.0, 0.0), WORLD))));
    let layout = DockingLayout::<&'static str>::new();
    let chrome = editor.chrome_shown();
    let open = create_signal(true);
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="graphing_calculator" size={SplitSize::First(280.0)}>
                <Show value={move || chrome.get() && open.get()}>
                    <DockPane id="equation_panel" locked=true>
                        <DockTab id=EQUATIONS title="Equations" on_close={move || open.set(false)}>
                            <Equations editor content />
                        </DockTab>
                    </DockPane>
                </Show>
                <DockPane id="graph_stage" locked=true bare=true>
                    <DockTab id=STAGE title="Graph">
                        <Plot @node_ref={&stage} editor content />
                    </DockTab>
                </DockPane>
            </DockSplit>
            <Show value={chrome}>
                <DockBar id="zoom" parent=STAGE placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::End)}>
                    <ZoomBar editor fitting />
                </DockBar>
            </Show>
            <Show value={move || chrome.get() && !open.get()}>
                <DockBar id="reopen" parent=STAGE placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::Start)}>
                    <DockBarSurface>
                        <DockBarGrip />
                        <Button icon_only=true glyph=ICON_PANEL_LEFT_OPEN label="Show equations" on_click={move || open.set(true)} />
                    </DockBarSurface>
                </DockBar>
            </Show>
        </Docking>
    }
}

#[component]
fn ZoomBar(editor: Editor, fitting: Fitting) -> NodeId {
    view! {
        <DockBarSurface>
            <DockBarGrip />
            <Button icon_only=true glyph=ICON_ZOOM_OUT label="Zoom out" on_click={move || editor.zoom(1.0 / ZOOM_STEP)} />
            <Button icon_only=true glyph=ICON_ZOOM_IN label="Zoom in" on_click={move || editor.zoom(ZOOM_STEP)} />
            <Button icon_only=true glyph=ICON_SCAN label="Fit graph" on_click={move || fitting.fit()} />
        </DockBarSurface>
    }
}

type GraphProjection = ContentProjection<GraphContent>;

#[cfg(test)]
mod tests;
