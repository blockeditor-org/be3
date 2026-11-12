use super::*;

const APPS_GRIP: &str = "#dock.bar.4398046511104.grip";
const WINDOWS_GRIP: &str = "#dock.bar.4398046511105.grip";

fn chosen(host: &mut HostTest, id: &str) -> bool {
    host.run(&["tree"])
        .lines()
        .any(|line| line.contains(&format!("#{id} ")) && line.contains("toggled=True"))
}

#[test]
fn the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu() {
    let mut host = HostTest::start(Options { desktop: true, ..Options::default() });
    host.wait("#desktop.windows.none");
    let hint = host.place("#desktop.windows.none");
    let grip = host.place(WINDOWS_GRIP);
    assert!(
        hint.2 > 4.0 * grip.2,
        "with no windows open the Windows bar keeps a chip's length and says so: {hint:?} beside its grip {grip:?}"
    );
    host.snapshot("the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu");

    host.run(&["click", WINDOWS_GRIP, "right"]);
    host.wait("#grip.dock.bar.beside.bottom");
    assert!(
        chosen(&mut host, "grip.dock.bar.beside.bottom"),
        "a secondary click on a bar's grip opens its menu, its place checked"
    );
    host.key("Escape");
    host.gone("#grip.dock.bar.beside.bottom");

    host.click(APPS_GRIP);
    host.wait("#grip.dock.bar.beside.ends");
    assert!(
        chosen(&mut host, "grip.dock.bar.beside.ends"),
        "a bar docked at an end checks the submenu that holds its place"
    );
    host.snapshot(
        "the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu_at_an_end",
    );
    host.key("Escape");
    host.gone("#grip.dock.bar.beside.ends");

    host.click("#desktop.apps");
    host.click("#desktop.apps.menu");
    host.wait("#host.menu.recovery-phrase");
    let tree = host.run(&["tree"]);
    assert!(
        tree.contains("#host.menu.inspector"),
        "every row of the host menu can be found by its id"
    );
    let scrolls = tree
        .lines()
        .take_while(|line| !line.contains("#host.menu.status"))
        .filter(|line| line.trim_start().starts_with("ScrollView"))
        .count();
    assert_eq!(scrolls, 1, "the host menu scrolls in one view: {tree}");
    host.close();
}
