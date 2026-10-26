use super::*;

#[test]
fn an_owned_window_is_listed_only_as_a_dialog_or_when_its_owner_is_hidden() {
    let main = window(1, "main");
    let panel = owned(2, &main, "panel");
    let dialog = RawWindow {
        class: "#32770".to_owned(),
        ..owned(3, &main, "dialog")
    };
    let framed = RawWindow {
        style: Style {
            dialog_frame: true,
            ..Style::default()
        },
        ..owned(4, &main, "framed")
    };
    let hidden_owner = RawWindow {
        visible: false,
        ..window(5, "hidden owner")
    };
    let app = owned(6, &hidden_owner, "app");
    let all = [main, panel, dialog, framed, hidden_owner, app];
    let index = Windows::new(&all);
    let listed: Vec<&str> = all
        .iter()
        .filter(|window| alt_tab(window, &index))
        .map(|window| window.title.as_str())
        .collect();
    assert_eq!(listed, ["main", "dialog", "framed", "app"]);
}
