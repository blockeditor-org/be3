use super::*;

use block_plugin_api::{HostTrayItem, TrayWatch, WindowsWatch};

#[test]
fn a_watched_value_wakes_only_the_watchers_of_its_own_watch() {
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let _root = RootScope::new();
    let (windows, tray) = (editor.watch::<WindowsWatch>(), editor.watch::<TrayWatch>());
    let runs = Rc::new(Cell::new(0));
    let counted = create_memo({
        let runs = Rc::clone(&runs);
        move || {
            runs.set(runs.get() + 1);
            tray.get().len()
        }
    });
    host.set_watched::<TrayWatch>(&vec![HostTrayItem::default()]);
    editor.begin_frame();
    assert_eq!(counted.get_untracked(), 1);
    let before = runs.get();

    host.set_watched::<WindowsWatch>(&Vec::new());
    editor.begin_frame();
    assert!(windows.get_untracked().is_empty());
    assert_eq!(counted.get_untracked(), 1);
    assert_eq!(
        runs.get(),
        before,
        "a change of the windows leaves the tray alone"
    );

    host.set_watched::<TrayWatch>(&vec![HostTrayItem::default()]);
    editor.begin_frame();
    assert_eq!(runs.get(), before, "the same tray again changes nothing");

    host.set_watched::<TrayWatch>(&Vec::new());
    editor.begin_frame();
    assert_eq!(counted.get_untracked(), 0);
    assert_eq!(
        runs.get(),
        before + 1,
        "a new tray reaches its watchers once"
    );
}
