use super::*;

#[test]
fn zz_caret_probe() {
    let mut host = HostTest::start(Options::default());
    for name in ["One", "Two"] {
        start_adding_to_my_files(&mut host, "Text");
        host.key("ctrl+a");
        host.type_text(name);
        host.key("Enter");
        host.wait(&format!("TreeItem \"{name}\""));
    }
    host.wait("Tab \"Two\" selected");
    host.click("Tab \"One\"");
    host.click("#text.surface");
    host.type_text("one");
    host.wait("one");
    host.click("Tab \"Two\"");
    host.click("#text.surface");
    host.type_text("two");
    host.wait("two");
    let dump = |host: &mut HostTest, label: &str| {
        let tree = host.tree();
        let lines: Vec<&str> = tree
            .lines()
            .filter(|line| line.contains("focused") || line.contains("MultilineTextInput") || line.contains("Tab \""))
            .collect();
        eprintln!("PROBE {label}:\n{}", lines.join("\n"));
    };
    dump(&mut host, "after typing in Two");
    host.key("ctrl+shift+Tab");
    host.pause(300);
    dump(&mut host, "right after ctrl+shift+tab");
    host.wait("MultilineTextInput value=\"one\" focused");
    host.type_text("1");
    host.pause(300);
    dump(&mut host, "after ctrl+shift+tab and typing");
    host.key("ctrl+Tab");
    host.pause(300);
    host.type_text("again");
    host.pause(300);
    dump(&mut host, "after ctrl+tab and typing");
    host.snapshot("zz_caret_probe");
    host.close();
}
