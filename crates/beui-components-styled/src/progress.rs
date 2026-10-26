use crate::frame::{ColorRole, Frame, Radius};
use beui_components_unstyled::{Progress as Bare, ProgressHandle};
use beui_macros::{component, view};

use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 6.0;
const RADIUS: f32 = 3.0;
const SEGMENT: f32 = 0.35;
const SLIVER: f32 = 0.001;

pub const INLINE_PROGRESS_WIDTH: f32 = 40.0;

type Spans = (f32, f32, f32);

#[component]
pub fn Progress(
    #[prop(default = None)] value: Prop<Option<f32>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = Extent::fraction(1.0))] width: Prop<Extent>,
    #[prop(default = ColorRole::Accent)] fill: Prop<ColorRole>,
    #[prop(default = ColorRole::Track)] track: Prop<ColorRole>,
) -> NodeId {
    view! {
        <Bare value label>|handle: ProgressHandle| {
            let ProgressHandle { value, phase } = handle;
            let spans = create_memo(move || match value.get() {
                Some(value) => (0.0, value, 1.0 - value),
                None => {
                    let phase = phase.get();
                    (phase * (1.0 - SEGMENT), SEGMENT, (1.0 - phase) * (1.0 - SEGMENT))
                }
            });
            let percent = move |span: fn(Spans) -> f32| {
                create_memo(move || ItemSize::Percent(span(spans.get()) * 100.0 + SLIVER))
            };
            let (width, fill, track) = (width.clone(), fill.clone(), track.clone());
            view! {
                <Frame width height=HEIGHT fill=track radius={Radius::custom(RADIUS)}>
                    <List direction=Direction::Horizontal spacing=0.0>
                        <Spacer @sizing={percent(|spans| spans.0)} />
                        <Frame
                            @sizing={percent(|spans| spans.1)}
                            fill
                            radius={Radius::custom(RADIUS)}
                        />
                        <Spacer @sizing={percent(|spans| spans.2)} />
                    </List>
                </Frame>
            }
        }</Bare>
    }
}
