use beui::styled::theme::{FONT_BODY, FONT_SMALL, RADIUS};
use beui::styled::Theme;
use beui::{pos2, Context, CursorIcon, FontId, Key, Rect, Vec2};

const THEME: Theme = Theme::DARK;
const BAND_HEIGHT: f32 = 36.0;
const BAND_PADDING: f32 = 12.0;
const STEP_GAP: f32 = 8.0;
const EXIT_PADDING: f32 = 10.0;
const SEPARATOR: f32 = 1.0;

pub(crate) struct Chrome {
    pub(crate) content: Rect,
    pub(crate) painted: Vec<Rect>,
    pub(crate) exit: bool,
}

impl Chrome {
    pub(crate) fn plain(rect: Rect) -> Self {
        Self {
            content: rect,
            painted: vec![rect],
            exit: false,
        }
    }
}

pub(crate) fn show(context: &Context, rect: Rect, trail: &[String], drawn: bool) -> Chrome {
    if !drawn || trail.is_empty() {
        return Chrome::plain(rect);
    }

    let band = Rect::from_min_size(rect.min, Vec2::new(rect.width(), BAND_HEIGHT));
    let painter = context.painter();
    painter.rect_filled(band, 0.0, THEME.surface);
    painter.rect_filled(
        Rect::from_min_size(
            pos2(band.left(), band.bottom() - SEPARATOR),
            Vec2::new(band.width(), SEPARATOR),
        ),
        0.0,
        THEME.border,
    );

    let font = FontId::proportional(FONT_BODY);
    let mut x = band.left() + BAND_PADDING;
    for (index, step) in trail.iter().enumerate() {
        let last = index + 1 == trail.len();
        if index > 0 {
            let galley = painter.layout(">", FontId::proportional(FONT_SMALL), f32::INFINITY);
            let width = galley.size().x;
            painter.galley(centered(band, x, galley.size()), galley, THEME.text_muted);
            x += width + STEP_GAP;
        }
        let galley = painter.layout(step.clone(), font, f32::INFINITY);
        let width = galley.size().x;
        let color = if last { THEME.text } else { THEME.text_muted };
        painter.galley(centered(band, x, galley.size()), galley, color);
        x += width + STEP_GAP;
    }

    let exit = exit_button(context, band);
    let content = Rect::from_min_max(pos2(rect.left(), band.bottom()), rect.max);
    Chrome {
        content,
        painted: vec![band, content],
        exit: exit || escaped(context),
    }
}

fn exit_button(context: &Context, band: Rect) -> bool {
    let painter = context.painter();
    let galley = painter.layout("Close", FontId::proportional(FONT_SMALL), f32::INFINITY);
    let size = galley.size();
    let button = Rect::from_min_size(
        pos2(
            band.right() - BAND_PADDING - size.x - EXIT_PADDING * 2.0,
            band.top() + (band.height() - size.y - EXIT_PADDING) / 2.0,
        ),
        Vec2::new(size.x + EXIT_PADDING * 2.0, size.y + EXIT_PADDING),
    );
    let hovered = context.input(|input| input.pointer.pos.is_some_and(|pos| button.contains(pos)));
    if hovered {
        context.set_cursor_icon(CursorIcon::PointingHand);
    }
    painter.rect_filled(
        button,
        RADIUS as f32,
        if hovered {
            THEME.surface_raised
        } else {
            THEME.surface
        },
    );
    painter.rect_stroke(button, RADIUS as f32, 1.0, THEME.border);
    painter.galley(
        pos2(
            button.left() + EXIT_PADDING,
            button.top() + (button.height() - size.y) / 2.0,
        ),
        galley,
        THEME.text,
    );
    hovered && context.input(|input| input.pointer.primary_released())
}

fn centered(band: Rect, x: f32, size: Vec2) -> beui::Pos2 {
    pos2(x, band.top() + (band.height() - size.y) / 2.0)
}

fn escaped(context: &Context) -> bool {
    context.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                beui::Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        })
    })
}
