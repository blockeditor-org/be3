use std::rc::Rc;
use std::time::Duration;

use beui_components_styled::{Button, ButtonVariant, Caption, Code, Heading, Theme};
use beui_core::base::TextAlign;
use beui_core::base::drawing::Draw;
use beui_core::color::Color32;
use beui_core::geometry::{Rect, pos2};
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_core::performance::{FramePerformance, PerformanceTimings, SAMPLE_CAPACITY};
use beui_view::reactive::{
    Align, Callback, Direction, Drawing, ForEach, Frame, Interactive, ItemSize, List, Memo, Prop,
    clone, component, create_memo, create_signal, untrack, view,
};

const THEME: Theme = Theme::DARK;
const CHART_HEIGHT: f32 = 72.0;
const BUDGET: Duration = Duration::from_micros(16_667);
const SPACING: f32 = 4.0;
const SWATCH_SIZE: f32 = 10.0;
const VALUE_WIDTH: f32 = 72.0;
const BAR_GAP: f32 = 1.0;
const FOCUS_WIDTH: f32 = 1.0;

#[derive(Clone, Copy)]
pub struct Phase {
    pub label: &'static str,
    pub color: Color32,
    pub read: fn(&PerformanceTimings) -> Duration,
}

pub const PHASES: [Phase; 5] = [
    Phase {
        label: "Layout",
        color: Color32::from_rgb(86, 156, 214),
        read: |timings| timings.layout,
    },
    Phase {
        label: "Interaction",
        color: Color32::from_rgb(197, 134, 192),
        read: |timings| timings.interaction,
    },
    Phase {
        label: "Paint",
        color: Color32::from_rgb(106, 176, 92),
        read: |timings| timings.paint,
    },
    Phase {
        label: "Accessibility",
        color: Color32::from_rgb(220, 170, 80),
        read: |timings| timings.accessibility,
    },
    Phase {
        label: "Other",
        color: Color32::from_rgb(128, 128, 128),
        read: |timings| timings.other,
    },
];

#[component]
pub fn FrameProfiler(
    frames: Prop<Vec<FramePerformance>>,
    #[prop(default = false)] paused: Prop<bool>,
    on_pause: Callback<bool>,
) -> NodeId {
    let frames = create_memo(move || frames.get());
    let (selected, set_selected) = create_signal(None::<u64>);
    let (hovered, set_hovered) = create_signal(None::<u64>);
    let (focused, set_focused) = create_signal(false);
    let shown = create_memo(clone!(frames selected hovered -> move || {
        let wanted = hovered.get().or(selected.get());
        frames.with(|frames| {
            wanted
                .and_then(|number| frames.iter().find(|frame| frame.number == number))
                .or(frames.last())
                .copied()
        })
    }));
    let chart = create_memo(clone!(frames selected hovered focused -> move || Chart {
        frames: frames.get(),
        selected: selected.get(),
        hovered: hovered.get(),
        focused: focused.get(),
    }));
    let draw = Prop::Dynamic(Rc::new(move || {
        let chart = chart.get();
        Rc::new(move |painter: &Painter, rect: Rect| chart.paint(painter, rect)) as Draw
    }));
    let summary = create_memo(clone!(shown -> move || frame_summary(shown.get())));
    let work = create_memo(clone!(shown -> move || frame_work(shown.get())));
    let pause_label = create_memo(clone!(paused -> move || match paused.get() {
        true => "Resume".to_owned(),
        false => "Pause".to_owned(),
    }));
    let pick = clone!(frames -> move |press: PointerPress| {
        frames.with_untracked(|frames| frame_at(frames, press.fraction.x).map(|frame| frame.number))
    });
    let click_pick = pick.clone();
    let step_frames = frames.clone();
    let (hover_cleared, picked) = (set_hovered.clone(), set_selected.clone());
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Heading @sizing=ItemSize::Percent(100.0) content="Frames" />
                <Button
                    @test_id={"inspector.performance.pause"}
                    label={pause_label}
                    variant=ButtonVariant::Secondary
                    on_click={move || on_pause.call(!untrack(|| paused.get()))}
                />
            </List>
            <Frame
                @test_id={"inspector.performance.frames"}
                height={Some(CHART_HEIGHT)}
                color={THEME.background}
            >
                <Interactive
                    focusable=true
                    on_focus_change={move |focused: bool| set_focused.set(focused)}
                    on_hover_move={move |press: PointerPress| set_hovered.set(pick(press))}
                    on_hover_change={move |hovering: bool| {
                        if !hovering {
                            hover_cleared.set(None);
                        }
                    }}
                    on_click_at={move |press: PointerPress| picked.set(click_pick(press))}
                    on_step={move |delta: f32| {
                        let current = selected.get_untracked();
                        let next = step_frames.with_untracked(|frames| stepped(frames, current, delta));
                        set_selected.set(next);
                    }}
                >
                    <Drawing draw />
                </Interactive>
            </Frame>
            <Caption
                content="Line: one 60 Hz frame. Click a frame, or focus and use the arrow keys."
                wrap=true
            />
            <Code @test_id={"inspector.performance.frame"} content={summary} />
            <Code content={work} color={THEME.text_muted} wrap=true />
            <ForEach keys={(0..PHASES.len()).collect::<Vec<_>>()}>
                {move |phase: usize| view! {
                    <PhaseRow phase shown={shown.clone()} />
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn PhaseRow(phase: usize, shown: Memo<Option<FramePerformance>>) -> NodeId {
    let phase = PHASES[phase];
    let value = create_memo(move || match shown.get() {
        Some(frame) => format!("{} ms", milliseconds((phase.read)(&frame.timings))),
        None => String::new(),
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Frame
                width={Some(SWATCH_SIZE)}
                height={Some(SWATCH_SIZE)}
                color={phase.color}
                radius=2
            />
            <Caption @sizing=ItemSize::Percent(100.0) content={phase.label.to_owned()} />
            <Code @sizing=ItemSize::Fixed(VALUE_WIDTH) content={value} align=TextAlign::End />
        </List>
    }
}

#[derive(Clone, PartialEq)]
struct Chart {
    frames: Vec<FramePerformance>,
    selected: Option<u64>,
    hovered: Option<u64>,
    focused: bool,
}

impl Chart {
    fn paint(&self, painter: &Painter, rect: Rect) {
        let grid = painter.pixel_grid();
        let slot = rect.width() / SAMPLE_CAPACITY as f32;
        let gap = if slot > 3.0 * BAR_GAP { BAR_GAP } else { 0.0 };
        let ceiling = self
            .frames
            .iter()
            .map(|frame| frame.timings.total)
            .max()
            .unwrap_or_default()
            .max(BUDGET * 2)
            .as_secs_f32();
        let height = |duration: Duration| duration.as_secs_f32() / ceiling * rect.height();
        let skipped = SAMPLE_CAPACITY.saturating_sub(self.frames.len());
        for (index, frame) in self.frames.iter().enumerate() {
            let left = rect.left() + (skipped + index) as f32 * slot;
            let right = left + slot - gap;
            let highlight = match Some(frame.number) {
                number if number == self.hovered => Some(THEME.hover),
                number if number == self.selected => Some(THEME.accent_soft),
                _ => None,
            };
            if let Some(color) = highlight {
                let band = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
                painter.rect_filled(grid.snap_rect(band), 0.0, color);
            }
            let mut bottom = rect.bottom();
            for phase in PHASES {
                let top = bottom - height((phase.read)(&frame.timings));
                let segment = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
                painter.rect_filled(grid.snap_rect(segment), 0.0, phase.color);
                bottom = top;
            }
        }
        let budget = rect.bottom() - height(BUDGET);
        painter.line(
            pos2(rect.left(), budget),
            pos2(rect.right(), budget),
            1.0,
            THEME.danger,
        );
        if self.focused {
            painter.rect_stroke(
                rect.shrink(FOCUS_WIDTH / 2.0),
                0.0,
                FOCUS_WIDTH,
                THEME.accent,
            );
        }
    }
}

fn frame_at(frames: &[FramePerformance], fraction: f32) -> Option<&FramePerformance> {
    let slot = (fraction.clamp(0.0, 1.0) * SAMPLE_CAPACITY as f32) as usize;
    let skipped = SAMPLE_CAPACITY.saturating_sub(frames.len());
    frames.get(slot.min(SAMPLE_CAPACITY - 1).checked_sub(skipped)?)
}

fn stepped(frames: &[FramePerformance], current: Option<u64>, delta: f32) -> Option<u64> {
    let last = frames.len().checked_sub(1)?;
    let index = current
        .and_then(|number| frames.iter().position(|frame| frame.number == number))
        .unwrap_or(last);
    let next = match delta < 0.0 {
        true => index.saturating_sub(1),
        false => (index + 1).min(last),
    };
    Some(frames[next].number)
}

fn frame_summary(frame: Option<FramePerformance>) -> String {
    match frame {
        Some(frame) => format!(
            "Frame {}: {} ms",
            frame.number,
            milliseconds(frame.timings.total)
        ),
        None => "Waiting for a document update".to_owned(),
    }
}

fn frame_work(frame: Option<FramePerformance>) -> String {
    match frame {
        Some(frame) => format!(
            "{} layout passes | paint {} | {} nodes | {} shapes",
            frame.layout_passes,
            if frame.painted { "ran" } else { "cached" },
            frame.nodes,
            frame.shapes
        ),
        None => String::new(),
    }
}

fn milliseconds(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1_000.0)
}
