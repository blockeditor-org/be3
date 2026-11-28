use std::rc::Rc;

use be_lib_graphing_calculator::Expression;
use be_sdk_beui::Editor;
use be_sdk_beui::be_block_content::presence::{PresenceColor, PresenceKind, pick_free_color};
use be_sdk_text::presence_color;
use beui::reactive::*;
use beui::styled::*;
use beui::{CanvasView, Color32, FontId, PointerPress, Pos2, Rect, TextLayout, pos2, vec2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{GraphProjection, WORLD};

const UNIT: f32 = 32.0;
const COLORS: [Color32; 6] = [
    Color32::from_rgb(49, 127, 222),
    Color32::from_rgb(221, 85, 108),
    Color32::from_rgb(36, 155, 116),
    Color32::from_rgb(160, 99, 221),
    Color32::from_rgb(193, 116, 39),
    Color32::from_rgb(37, 149, 177),
];

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
struct GraphPointer {
    x: f32,
    y: f32,
    color: PresenceColor,
}

impl PresenceKind for GraphPointer {
    const ID: Uuid = Uuid::from_u128(0x156a_8611_4175_4433_ac14_332a_3f7b_91ac);
}

#[component]
pub(super) fn Plot(editor: Editor, content: GraphProjection) -> NodeId {
    let theme = use_theme();
    let view = editor.canvas();
    let visible = editor.presence_visible();
    let peers = editor.peers::<GraphPointer>();
    let color = pick_free_color(peers.get_untracked().into_iter().map(|(_, peer)| peer.color));
    let hovered = create_signal(None::<GraphPointer>);
    create_effect(move || {
        let pointer = hovered.get().filter(|_| visible.get());
        editor.show(pointer.as_ref());
    });
    on_cleanup(move || editor.show::<GraphPointer>(None));

    let changed = move |press: PointerPress| {
        let at = camera(view.get_untracked(), press.pos - press.local).to_canvas(press.pos);
        if at.x.is_finite() && at.y.is_finite() {
            hovered.set(Some(GraphPointer { x: at.x, y: -at.y, color }));
        }
    };
    let expressions = content.project(|content| {
        Rc::new(
            content.root().equations.iter().filter_map(|row| {
                Expression::parse(&row.expression.to_str_lossy())
                    .ok()
                    .map(|expression| (row.id.as_uuid().as_u128() as usize % COLORS.len(), expression))
            }).collect::<Vec<_>>()
        )
    });
    let draw = move || {
        let view = view.get();
        let theme = theme.get();
        let expressions = expressions.get();
        let peers = if visible.get() { peers.get() } else { Vec::new() };
        Rc::new(move |painter: &Painter, rect: Rect| {
            let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
            painter.rect_filled(rect, 0.0, theme.surface);
            let camera = camera(view, rect.min.to_vec2());
            let bounds = camera.rect_to_canvas(rect);
            if camera.scale <= 0.0 || !bounds.width().is_finite() || !bounds.height().is_finite() {
                return;
            }
            draw_grid(&painter, rect, camera, theme.border, theme.text_muted);
            let columns = (rect.width() / 2.0).ceil().clamp(1.0, 2048.0) as usize;
            for (color, expression) in expressions.iter() {
                for [a, b] in expression.segments(
                    f64::from(bounds.left()), f64::from(bounds.right()), columns, f64::from(camera.scale.recip()),
                ) {
                    if let Some([a, b]) = clip_segment(a, b, bounds) {
                        painter.line(camera.to_screen(a), camera.to_screen(b), 2.0, COLORS[*color]);
                    }
                }
            }
            for (_, pointer) in &peers {
                let at = pos2(pointer.x, -pointer.y);
                if bounds.contains(at) {
                    let dot = Rect::from_center_size(camera.to_screen(at), vec2(9.0, 9.0));
                    painter.rect_filled(dot, 4.5, presence_color(pointer.color));
                }
            }
        }) as Draw
    };

    view! {
        <Interactive
            on_press={move |_: PointerPress| editor.pan_drag()}
            on_hover_move={changed}
            on_hover_change={move |inside: bool| { if !inside { hovered.set(None); } }}
            @test_id={"graphing-calculator.stage"}
        >
            <Drawing draw={draw} />
        </Interactive>
    }
}

fn camera(view: Option<CanvasView>, fallback: beui::Vec2) -> CanvasView {
    let view = view.unwrap_or_else(|| CanvasView::new(pos2(fallback.x, fallback.y), 1.0));
    CanvasView::new(view.origin + WORLD * (view.scale * 0.5), view.scale * UNIT)
}

fn draw_grid(painter: &Painter, rect: Rect, view: CanvasView, grid: Color32, axis: Color32) {
    let target = 68.0 / view.scale;
    let decade = 10.0_f32.powf(target.log10().floor());
    let step = decade * [1.0, 2.0, 5.0, 10.0].into_iter()
        .find(|multiple| decade * multiple >= target)
        .unwrap_or(10.0);
    if !step.is_finite() || step <= 0.0 {
        return;
    }
    let bounds = view.rect_to_canvas(rect);
    let font = FontId::proportional(11.0);
    for horizontal in [false, true] {
        let (start, end) = if horizontal {
            (bounds.top(), bounds.bottom())
        } else {
            (bounds.left(), bounds.right())
        };
        let first = (start / step).ceil();
        let count = ((end / step).floor() - first + 1.0).clamp(0.0, 256.0) as usize;
        for index in 0..count {
            let value = (first + index as f32) * step;
            let at = if horizontal {
                view.to_screen(pos2(0.0, value)).y
            } else {
                view.to_screen(pos2(value, 0.0)).x
            };
            let (from, to) = if horizontal {
                (pos2(rect.left(), at), pos2(rect.right(), at))
            } else {
                (pos2(at, rect.top()), pos2(at, rect.bottom()))
            };
            painter.line(from, to, 1.0, if value == 0.0 { axis } else { grid });
            if value == 0.0 && horizontal {
                continue;
            }
            let value = if horizontal { -value } else { value };
            let label = if value.abs() >= 1e6 || step < 0.001 {
                format!("{value:.2e}")
            } else {
                let digits = (-step.log10().floor()).max(0.0) as usize;
                format!("{value:.digits$}")
            };
            let text = painter.lay_out_text(&label, font, TextLayout::wrapped(96.0));
            let at = if horizontal {
                pos2(view.origin.x.max(rect.left()) + 4.0, at + 2.0)
            } else {
                pos2(at + 4.0, view.origin.y.min(rect.bottom() - 16.0).max(rect.top()) + 2.0)
            };
            painter.draw_text(at, &text, axis);
        }
    }
}

fn clip_segment(a: (f64, f64), b: (f64, f64), bounds: Rect) -> Option<[Pos2; 2]> {
    let top = -f64::from(bounds.bottom());
    let bottom = -f64::from(bounds.top());
    if a.1.max(b.1) < top || a.1.min(b.1) > bottom {
        return None;
    }
    let clip = |(x, y): (f64, f64)| {
        let clipped = y.clamp(top, bottom);
        let x = if clipped == y { x } else { a.0 + (b.0 - a.0) * ((clipped - a.1) / (b.1 - a.1)) };
        pos2(x as f32, -clipped as f32)
    };
    Some([clip(a), clip(b)])
}
