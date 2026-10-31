use super::*;

const PROGRAMS: [(&str, &str, &str); 6] = [
    ("firefox", "Firefox", "Rust 2024 edition guide"),
    ("org.kde.kate", "Kate", "quarterly-report-draft.md"),
    ("foot", "Foot", "~/src/be3"),
    ("org.gnome.Nautilus", "Files", "Downloads"),
    ("thunderbird", "Thunderbird", "Inbox - mia@example.org"),
    ("org.inkscape.Inkscape", "Inkscape", "poster-final-v3.svg"),
];

fn realistic(id: u64, focused: Option<u64>) -> HostWindow {
    let (app_id, name, title) = PROGRAMS[(id as usize - 1) % PROGRAMS.len()];
    HostWindow {
        app_id: app_id.to_owned(),
        name: name.to_owned(),
        title: title.to_owned(),
        ..host_window(id, focused == Some(id))
    }
}

fn show(fixture: &mut Fixture, count: u64, focused: Option<u64>) {
    let windows: Vec<HostWindow> = (1..=count).map(|id| realistic(id, focused)).collect();
    fixture.show_windows(&windows);
}

fn chip(id: u64) -> String {
    format!("desktop.windows.{id}")
}

fn shown_within(fixture: &Fixture, id: u64, bar: Rect) -> bool {
    fixture.test.shown(&chip(id)) && bar.contains_rect(fixture.test.rect_of(&chip(id)))
}

#[test]
fn the_windows_bar_shares_its_length_between_many_chips() {
    let mut fixture = Fixture::new();
    fixture.settle();
    show(&mut fixture, 2, Some(1));
    let first = fixture.test.rect_of(&chip(1));
    let second = fixture.test.rect_of(&chip(2));
    assert!(
        (first.width() - second.width()).abs() < 0.5,
        "chips are as wide as each other, whatever their titles: {first:?} {second:?}"
    );

    show(&mut fixture, 5, Some(1));
    let bar = fixture.test.rect_of("desktop.bar.windows");
    let widths: Vec<f32> = (1..=5)
        .map(|id| fixture.test.rect_of(&chip(id)).width())
        .collect();
    assert!(
        widths.iter().all(|width| (width - widths[0]).abs() <= 1.0) && widths[0] < first.width(),
        "more chips than fit at full width shrink evenly: {widths:?}"
    );
    assert!(
        (1..=5).all(|id| shown_within(&fixture, id, bar)),
        "and every chip still fits on the bar"
    );
    fixture.test.snapshot("the_windows_bar_shrinking_its_chips");

    show(&mut fixture, 12, Some(1));
    let bar = fixture.test.rect_of("desktop.bar.windows");
    let icon = fixture.test.rect_of(&chip(1));
    assert!(
        icon.width() < widths[0] / 2.0,
        "past the least a title can show in, chips show only their icons: {icon:?}"
    );
    assert!(
        (1..=12).all(|id| shown_within(&fixture, id, bar)),
        "and every icon fits on the bar"
    );
    fixture.test.snapshot("the_windows_bar_with_only_icons");

    show(&mut fixture, 40, Some(40));
    let bar = fixture.test.rect_of("desktop.bar.windows");
    assert!(
        shown_within(&fixture, 40, bar),
        "past that the chips scroll, keeping the focused one on the bar"
    );
    assert!(
        !shown_within(&fixture, 1, bar),
        "with the first ones scrolled out of the way"
    );
    show(&mut fixture, 40, Some(2));
    assert!(
        shown_within(&fixture, 2, bar),
        "focusing a window scrolled away brings its chip back onto the bar"
    );
    fixture
        .test
        .snapshot("the_windows_bar_scrolling_to_the_focused_chip");

    fixture.bar_menu(WINDOWS_BAR, &["beside.left"]);
    show(&mut fixture, 40, Some(39));
    let bar = fixture.test.rect_of("desktop.bar.windows");
    assert!(
        bar.height() > bar.width(),
        "the Windows bar runs down the left: {bar:?}"
    );
    assert!(
        shown_within(&fixture, 39, bar),
        "a bar down the side scrolls to the focused chip too"
    );
    let grip = fixture
        .test
        .rect_of(&format!("dock.bar.{}.grip", SHELL_BARS + WINDOWS_BAR));
    assert!(
        bar.contains_rect(grip) && grip.top() - bar.top() < grip.height(),
        "only the chips scroll: the grip stays at the top of the bar: {grip:?} in {bar:?}"
    );
    fixture
        .test
        .snapshot("the_windows_bar_down_the_left_with_many_windows");
}
