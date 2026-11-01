use std::time::{Duration, Instant};

use block_editor_beui::beui::{
    self, Input, Pen, PenPhase, PointerButton, Pos2, Rect, Vec2, native::FramePerformance,
};
use block_ui_test::BeuiTest;
use infinite_canvas::app::CanvasApp;
use infinite_canvas_types::CanvasContent;
use infinite_canvas_types::{
    Canvas, CanvasColor, CanvasEntity, CanvasFill, CanvasPathPoint, CanvasPoint, CanvasShape,
    CanvasTransform,
};
use uuid::Uuid;

const SHAPES: usize = 10_000;
const FEW: usize = 100;
const COLUMNS: usize = 100;
const SPACING: Vec2 = Vec2::new(80.0, 60.0);
const FRAMES: usize = 240;
const WARMUP: usize = 20;
const STROKES: usize = 8;
const STROKE_MOVES: usize = 60;
const LONG_STROKE_MOVES: usize = 2000;
const LONG_STROKE_ENDS: usize = 200;
const MOST_FRAMES_TO_INK: usize = 8;

fn shape(index: usize) -> CanvasEntity {
    let (row, column) = (index / COLUMNS, index % COLUMNS);
    let center = CanvasPoint::new(column as f32 * SPACING.x, row as f32 * SPACING.y);
    let world = |x: f32, y: f32| CanvasPoint::new(center.x + x * 60.0, center.y + y * 40.0);
    let wave: Vec<CanvasPoint> = (0..12)
        .map(|step| {
            let along = step as f32 / 11.0;
            world(along - 0.5, (along * 9.0).sin() * 0.5)
        })
        .collect();
    let transform = CanvasTransform::new(center, CanvasPoint::new(60.0, 40.0), 0.0);
    let mut entity = match index % 4 {
        0 => CanvasEntity::new(CanvasShape::Rectangle, transform),
        1 => CanvasEntity::new(CanvasShape::Ellipse, transform),
        2 => CanvasEntity::path(&[world(-0.5, 0.5), world(0.0, -0.5), world(0.5, 0.5)]),
        _ => {
            let mut freehand = CanvasEntity::new(CanvasShape::Freehand, transform);
            freehand.set_world_points(wave.iter().copied().map(CanvasPathPoint::new).collect());
            freehand.pressures = wave
                .iter()
                .map(|point| 0.5 + (point.y - center.y) / 80.0)
                .collect();
            freehand
        }
    };
    entity.id = Uuid::from_u128(index as u128 + 1);
    entity.style.seed = index as u32;
    if index % 4 < 2 {
        entity.style.fill = CanvasFill::Solid;
        entity.style.fill_color = CanvasColor::Rgba {
            red: 40,
            green: 90 + (column % 8 * 20) as u8,
            blue: 160,
            alpha: 255,
        };
    }
    entity
}

fn open(count: usize) -> BeuiTest<CanvasApp> {
    let mut test = BeuiTest::<CanvasApp>::editing(Uuid::new_v4()).in_viewport();
    beui::native::verify_paint(false);
    let canvas = Canvas::with_entities((0..count).map(shape));
    test.hold(None, CanvasContent::new(&canvas));
    test.store().keep_history::<CanvasContent>(None);
    test.run();
    test.settle();
    test
}

fn world_size(count: usize) -> Vec2 {
    let rows = count.div_ceil(COLUMNS);
    Vec2::new(
        COLUMNS.min(count) as f32 * SPACING.x,
        rows as f32 * SPACING.y,
    )
}

#[derive(Clone, Copy)]
struct Frame {
    wall: Duration,
    latest: FramePerformance,
}

fn frame(test: &mut BeuiTest<CanvasApp>, events: Vec<Input>) -> Frame {
    let started = Instant::now();
    test.step(events);
    let wall = started.elapsed();
    Frame {
        wall,
        latest: test.document().performance().latest,
    }
}

fn percentile<T: Copy + Ord>(mut values: Vec<T>, share: f64) -> T {
    values.sort();
    values[((values.len() - 1) as f64 * share).round() as usize]
}

fn report(name: &str, metric: &str, frames: &[Frame]) {
    let wall = |share| percentile(frames.iter().map(|frame| frame.wall).collect(), share);
    bench::time(&format!("{metric}.p50"), wall(0.5));
    let median = |pick: fn(&Frame) -> Duration| percentile(frames.iter().map(pick).collect(), 0.5);
    let most = |pick: fn(&FramePerformance) -> usize| {
        frames
            .iter()
            .map(|frame| pick(&frame.latest))
            .max()
            .unwrap_or(0)
    };
    println!(
        "{name:<22} wall p50 {:>8.2?} p90 {:>8.2?} max {:>8.2?}  [layout {:>7.2?} paint {:>7.2?} interact {:>7.2?} accessibility {:>7.2?}]  most measured {:>5} placed {:>5} painted {:>5} described {:>5} walked {:>5}",
        wall(0.5),
        wall(0.9),
        wall(1.0),
        median(|frame| frame.latest.timings.layout),
        median(|frame| frame.latest.timings.paint),
        median(|frame| frame.latest.timings.interaction),
        median(|frame| frame.latest.timings.accessibility),
        most(|latest| latest.work.measured),
        most(|latest| latest.work.placed),
        most(|latest| latest.work.painted_nodes),
        most(|latest| latest.work.described_nodes),
        most(|latest| latest.work.walked_nodes),
    );
}

fn shown(test: &BeuiTest<CanvasApp>) -> Pos2 {
    test.rect_of("infinite-canvas.canvas").center()
}

fn camera(test: &mut BeuiTest<CanvasApp>, shown: Pos2, center: Pos2, scale: f32) {
    let content = test
        .intrinsic_size()
        .unwrap_or_default()
        .max(test.rect().size());
    let size = content * scale;
    test.set_view(
        Rect::from_center_size(shown - center.to_vec2() * scale, size),
        scale,
    );
}

fn pan(count: usize, scale: f32) -> Vec<Frame> {
    let mut test = open(count);
    let shown = shown(&test);
    let world = world_size(count);
    let start = Pos2::new(world.x * 0.25, world.y * 0.25);
    camera(&mut test, shown, start, scale);
    test.run();
    let reach = world * 0.5;
    (0..WARMUP + FRAMES)
        .map(|index| {
            let along = index as f32 / (WARMUP + FRAMES) as f32;
            let angle = along * std::f32::consts::TAU;
            camera(
                &mut test,
                shown,
                start + Vec2::new(reach.x * along, reach.y * 0.2 * angle.sin()),
                scale,
            );
            frame(&mut test, Vec::new())
        })
        .skip(WARMUP)
        .collect()
}

fn zoom(count: usize) -> Vec<Frame> {
    let mut test = open(count);
    let shown = shown(&test);
    let world = world_size(count);
    let center = Pos2::new(world.x * 0.5, world.y * 0.5);
    let fit = (test.rect().width() / world.x).min(test.rect().height() / world.y);
    camera(&mut test, shown, center, fit);
    test.run();
    (0..WARMUP + FRAMES)
        .map(|index| {
            let along = index as f32 / (WARMUP + FRAMES) as f32;
            let scale = fit * 8.0f32.powf(0.5 - 0.5 * (along * std::f32::consts::TAU).cos());
            camera(&mut test, shown, center, scale);
            frame(&mut test, Vec::new())
        })
        .skip(WARMUP)
        .collect()
}

fn pen(phase: PenPhase, pos: Pos2, pressure: f32) -> Input {
    Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure,
            ..Pen::default()
        },
    }
}

fn inked(test: &BeuiTest<CanvasApp>, at: Pos2) -> bool {
    test.drawn()
        .and_then(|output| output.damaged())
        .is_some_and(|damage| {
            damage
                .rects()
                .iter()
                .any(|rect| rect.expand(1.0).contains(at))
        })
}

#[inline(never)]
fn lift(test: &mut BeuiTest<CanvasApp>, at: Pos2) -> Frame {
    frame(
        test,
        vec![pen(PenPhase::Up(PointerButton::Primary), at, 0.0)],
    )
}

struct Ink {
    moves: Vec<Frame>,
    frames_to_ink: Vec<usize>,
    lifts: Vec<Frame>,
}

fn ink(count: usize) -> Ink {
    let mut test = open(count);
    let shown = shown(&test);
    let world = world_size(count);
    camera(
        &mut test,
        shown,
        Pos2::new(world.x * 0.5, world.y * 0.5),
        1.0,
    );
    test.run();
    test.click("infinite-canvas.dock.tool.Pen");
    test.run();
    test.settle();
    let canvas = test.rect_of("infinite-canvas.canvas");
    let mut ink = Ink {
        moves: Vec::new(),
        frames_to_ink: Vec::new(),
        lifts: Vec::new(),
    };
    for stroke in 0..STROKES {
        camera(
            &mut test,
            shown,
            Pos2::new(world.x * 0.5, world.y * 0.5),
            1.0,
        );
        test.run();
        test.settle();
        let from = canvas.center() + Vec2::new(-150.0, (stroke as f32 - 4.0) * 25.0);
        let at = |step: usize| {
            let along = step as f32 / STROKE_MOVES as f32;
            from + Vec2::new(300.0 * along, 20.0 * (along * 12.0).sin())
        };
        frame(&mut test, vec![pen(PenPhase::Move, from, 0.0)]);
        for step in 0..=STROKE_MOVES {
            let phase = match step {
                0 => PenPhase::Down(PointerButton::Primary),
                _ => PenPhase::Move,
            };
            let delivered = frame(&mut test, vec![pen(phase, at(step), 0.6)]);
            let mut waited = 0;
            while !inked(&test, at(step)) && waited < MOST_FRAMES_TO_INK {
                frame(&mut test, Vec::new());
                waited += 1;
            }
            ink.moves.push(delivered);
            ink.frames_to_ink.push(waited);
        }
        ink.lifts.push(lift(&mut test, at(STROKE_MOVES)));
        test.run();
        test.settle();
    }
    ink
}

#[inline(never)]
fn long_move(test: &mut BeuiTest<CanvasApp>, at: Pos2) -> Frame {
    frame(test, vec![pen(PenPhase::Move, at, 0.6)])
}

fn long_ink(count: usize) -> (Vec<Frame>, Vec<Frame>) {
    let mut test = open(count);
    let shown = shown(&test);
    let world = world_size(count);
    camera(
        &mut test,
        shown,
        Pos2::new(world.x * 0.5, world.y * 0.5),
        1.0,
    );
    test.run();
    test.click("infinite-canvas.dock.tool.Pen");
    test.run();
    test.settle();
    let canvas = test.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(200.0, 0.0);
    let at = |step: usize| {
        let along = step as f32 / LONG_STROKE_MOVES as f32;
        from + Vec2::new(400.0 * along, 150.0 * (along * 40.0).sin())
    };
    frame(&mut test, vec![pen(PenPhase::Move, from, 0.0)]);
    frame(
        &mut test,
        vec![pen(PenPhase::Down(PointerButton::Primary), from, 0.6)],
    );
    let moves: Vec<Frame> = (1..=LONG_STROKE_MOVES)
        .map(|step| long_move(&mut test, at(step)))
        .collect();
    lift(&mut test, at(LONG_STROKE_MOVES));
    (
        moves[..LONG_STROKE_ENDS].to_vec(),
        moves[moves.len() - LONG_STROKE_ENDS..].to_vec(),
    )
}

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    }
}

fn drag(count: usize) -> Vec<Frame> {
    let mut test = open(count);
    let shown = shown(&test);
    let rows = count.div_ceil(COLUMNS);
    let index = rows / 2 * COLUMNS + 48;
    let held = shape(index).transform.center;
    let canvas = test.rect_of("infinite-canvas.canvas");
    let from = canvas.center();
    let at = |along: f32| from + Vec2::new(200.0 * along, 40.0 * (along * 6.0).sin());
    let mut moves = Vec::new();
    for _ in 0..STROKES {
        camera(&mut test, shown, Pos2::new(held.x, held.y), 1.0);
        test.run();
        test.settle();
        for (start, end) in [(0.0, 1.0), (1.0, 0.0)] {
            let grabbed = at(start);
            frame(
                &mut test,
                vec![Input::PointerMoved(grabbed), button(grabbed, true)],
            );
            for step in 1..=STROKE_MOVES {
                let along = step as f32 / STROKE_MOVES as f32;
                let pointer = at(start + (end - start) * along);
                moves.push(frame(&mut test, vec![Input::PointerMoved(pointer)]));
            }
            frame(&mut test, vec![button(at(end), false)]);
            let moved = test
                .content::<CanvasContent>(None)
                .root()
                .entities()
                .into_iter()
                .find(|entity| entity.id == shape(index).id)
                .map(|entity| entity.transform.center);
            let expected = at(end) - at(0.0);
            assert!(
                moved.is_some_and(|moved| (moved.x - held.x - expected.x).abs() < 0.5
                    && (moved.y - held.y - expected.y).abs() < 0.5),
                "the drag moves the shape {held:?} by {expected:?}, but it is at {moved:?}"
            );
        }
    }
    moves
}

fn main() {
    let only = std::env::args().nth(1);
    let wanted = |name: &str| only.as_deref().is_none_or(|only| name.contains(only));
    let counts: Vec<usize> = match std::env::args().nth(2).and_then(|count| count.parse().ok()) {
        Some(count) => vec![count],
        None => vec![FEW, SHAPES],
    };
    for count in counts {
        if wanted("open") {
            let started = Instant::now();
            drop(open(count));
            let took = started.elapsed();
            bench::time(&format!("{count}-shapes.open"), took);
            println!("open {count:>6} shapes        {took:>8.2?}");
        }
        if wanted("pan") {
            report(
                &format!("pan {count} at 1"),
                &format!("{count}-shapes.pan-at-1"),
                &pan(count, 1.0),
            );
            report(
                &format!("pan {count} at 0.1"),
                &format!("{count}-shapes.pan-at-0.1"),
                &pan(count, 0.1),
            );
        }
        if wanted("zoom") {
            report(
                &format!("zoom {count}"),
                &format!("{count}-shapes.zoom"),
                &zoom(count),
            );
        }
        if wanted("long") {
            let (first, last) = long_ink(count);
            report(
                &format!("long ink first {count}"),
                &format!("{count}-shapes.long-ink-first"),
                &first,
            );
            report(
                &format!("long ink last {count}"),
                &format!("{count}-shapes.long-ink-last"),
                &last,
            );
        }
        if wanted("drag") {
            report(
                &format!("drag {count}"),
                &format!("{count}-shapes.drag"),
                &drag(count),
            );
        }
        if wanted("ink") {
            let ink = ink(count);
            report(
                &format!("ink move {count}"),
                &format!("{count}-shapes.pen-move"),
                &ink.moves,
            );
            report(
                &format!("ink lift {count}"),
                &format!("{count}-shapes.pen-lift"),
                &ink.lifts,
            );
            let late = ink
                .frames_to_ink
                .iter()
                .filter(|waited| **waited > 0)
                .count();
            let worst = ink.frames_to_ink.iter().max().copied().unwrap_or(0);
            bench::count(&format!("{count}-shapes.frames-to-ink"), worst as u64);
            println!(
                "ink {count:>6} shapes        {} samples, {late} needed a later frame, worst {} frames after delivery",
                ink.frames_to_ink.len(),
                worst,
            );
        }
    }
}
