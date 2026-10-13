use std::time::Duration;

use beui::reactive::{Frame, build, view};
use beui::styled::Theme;
use beui::{Event, FramePerformance, FrameProfiler, PerformanceTimings};

use super::*;

fn frame(number: u64) -> FramePerformance {
    let wave = number % 12;
    let spike = number.is_multiple_of(37);
    let layout = Duration::from_micros(1_500 + wave * 300 + if spike { 14_000 } else { 0 });
    let interaction = Duration::from_micros(400 + (number % 5) * 120);
    let paint = Duration::from_micros(2_500 + (number % 7) * 600);
    let accessibility = Duration::from_micros(300 + (number % 3) * 200);
    let other = Duration::from_micros(500);
    FramePerformance {
        number,
        timings: PerformanceTimings {
            total: layout + interaction + paint + accessibility + other,
            layout,
            interaction,
            paint,
            accessibility,
            other,
        },
        layout_passes: usize::from(!number.is_multiple_of(4)),
        painted: !number.is_multiple_of(6),
        nodes: 412,
        shapes: 1_288,
        ..FramePerformance::default()
    }
}

#[test]
fn the_frame_profiler_stacks_each_frames_phases() {
    let frames: Vec<FramePerformance> = (1..=90).map(frame).collect();
    let document = build(move || {
        view! {
            <Frame color={Theme::DARK.surface} padding_horizontal=8.0 padding_vertical=8.0>
                <FrameProfiler frames={frames.clone()} />
            </Frame>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(360.0, 340.0));
    let chart = test.rect_of("inspector.performance.frames");
    test.click_at(pos2(chart.left() + chart.width() * 0.7, chart.center().y));
    test.frame(vec![Event::PointerMoved(Pos2::ZERO)]);
    test.snapshot("frame_profiler");
}
