use super::*;

#[test]
fn alt_tab_lists_plain_windows_and_skips_tool_cloaked_and_hidden_ones() {
    let plain = window(1, "plain");
    let tool = RawWindow {
        style: Style {
            tool_window: true,
            ..Style::default()
        },
        ..window(2, "tool")
    };
    let tool_app = RawWindow {
        style: Style {
            tool_window: true,
            app_window: true,
            ..Style::default()
        },
        ..window(3, "tool app")
    };
    let cloaked = RawWindow {
        cloaked: true,
        ..window(4, "cloaked")
    };
    let hidden = RawWindow {
        visible: false,
        ..window(5, "hidden")
    };
    let empty = RawWindow {
        frame: Rect::new(10, 10, 10, 10),
        ..window(6, "empty")
    };
    let minimized = RawWindow {
        minimized: true,
        frame: Rect::new(-32000, -32000, -32000, -32000),
        ..window(7, "minimized")
    };
    let shell = RawWindow {
        class: "Progman".to_owned(),
        ..window(8, "Program Manager")
    };
    let no_activate = RawWindow {
        style: Style {
            no_activate: true,
            ..Style::default()
        },
        ..window(9, "no activate")
    };
    let all = [
        plain,
        tool,
        tool_app,
        cloaked,
        hidden,
        empty,
        minimized,
        shell,
        no_activate,
    ];
    let index = Windows::new(&all);
    let listed: Vec<String> = all
        .iter()
        .filter(|window| alt_tab(window, &index))
        .map(|window| title_of(window.handle))
        .collect();
    assert_eq!(listed, ["plain", "tool app", "minimized"]);
}
