use super::*;
use crate::display::arrangement::{View, carry_places, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{PointerButton, Pos2, Rect, pos2, vec2};

#[test]
fn a_carried_display_grabbed_by_its_corner_snaps_by_the_rect_it_is_drawn_at() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();

    let laid = tiles(&displays, &Arrangement::default());
    let rects: Vec<Rect> = laid.iter().map(|tile| tile.rect).collect();
    // From the lift, the area fits every place the monitor can go, so it is drawn smaller.
    let carry = View::of(&[rects.clone(), carry_places(&laid, MONITOR)].concat());
    let desk = settings.editor.rect_of("display.arrangement");
    let on_desk = |point: Pos2| desk.min + carry.to_area(point).to_vec2();
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    // Grabbed just inside its bottom right corner, far from its centre, and held a few
    // points off the place left of the laptop where their tops line up: the monitor drawn
    // there snaps onto it. Held as lifted, larger, it would be measured from a centre well
    // past SNAP from it.
    let lifted = settings.editor.rect_of("display.arrangement.DP-1");
    let fill = tile_fill(&settings.editor, "display.arrangement.DP-1");
    let from = lifted.max - vec2(4.0, 4.0);
    let grabbed = vec2(
        (from.x - lifted.min.x) / lifted.width(),
        (from.y - lifted.min.y) / lifted.height(),
    );
    let place = Rect::from_min_size(
        pos2(laptop.left() - monitor.width(), laptop.top()),
        monitor.size(),
    );
    let shown = Rect::from_min_max(on_desk(place.min), on_desk(place.max));
    let to = shown.min + shown.size() * grabbed + vec2(3.0, 2.0);
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + (to - from) / 2.0);
    settings.editor.hover(to);
    settings.run();
    let carried = drawn_tile(&settings.editor, fill);
    assert!(
        (carried.min - shown.min).length() < 1.0,
        "the monitor snaps onto the place: {carried:?}, not {shown:?}"
    );
    settings.editor.up(to, PointerButton::Primary);
    settings.run();

    let arrangement = settings.here().arrangement();
    assert_eq!(
        arrangement
            .place(MONITOR)
            .map(|(x, _)| x + monitor.width() as i32),
        arrangement.place(LAPTOP).map(|(x, _)| x),
        "the monitor meets the laptop's left edge"
    );
    assert_eq!(
        arrangement.place(MONITOR).map(|(_, y)| y),
        arrangement.place(LAPTOP).map(|(_, y)| y),
        "with their tops lined up"
    );
}
