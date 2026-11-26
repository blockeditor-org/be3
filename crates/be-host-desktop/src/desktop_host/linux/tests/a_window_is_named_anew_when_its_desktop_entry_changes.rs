use std::cell::RefCell;
use std::rc::Rc;

use beui::Vec2;

use super::*;

/// A window not launched from a desktop entry is named by the installed entry its app id
/// names, as the programs' list read them last: installed after the window opened, or
/// renamed, the entry renames the listed window once the list is read again, and what shows
/// its name (its tab, through `window_status`) hears of it with no frame asking again.
#[test]
fn a_window_is_named_anew_when_its_desktop_entry_changes() {
    let root = std::env::temp_dir().join(format!("be-host-desktop-named-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _removed = Removed(root.clone());
    let entry = root.join("applications/org.example.Notes.desktop");
    std::fs::create_dir_all(entry.parent().expect("an entry has a folder"))
        .expect("the folder is made");
    let read = || {
        LinuxPrograms::of(Environment {
            data_dirs: vec![root.clone()],
            ..Environment::default()
        })
        .adopted();
    };

    let _root = RootScope::new();
    Linux::create();
    let windows = windows().expect("the window list is made");
    windows.list_for_testing(vec![be_host_wayland::WindowInfo {
        id: WindowId(1),
        title: "notes.txt".to_owned(),
        program_id: "org.example.Notes".to_owned(),
        parent: None,
        modal: false,
        size: Vec2::new(400.0, 300.0),
        drawn: Vec2::new(400.0, 300.0),
        min_size: Vec2::ZERO,
        max_size: Vec2::ZERO,
        maximized: false,
        fullscreen: None,
        responding: true,
        icon: None,
        inhibits_shortcuts: false,
        pointer: None,
        launch: None,
        command: Vec::new(),
    }]);
    let named = Rc::new(RefCell::new(Vec::new()));
    create_effect({
        let named = named.clone();
        move || {
            let status = Linux::window_status(HostWindowId(1)).expect("the window is listed");
            named.borrow_mut().push(status.name);
        }
    });
    read();
    flush();
    let names = || named.borrow().clone();
    assert_eq!(
        names(),
        ["Notes"],
        "with no entry installed for its app id, the window is named from the id"
    );

    let write = |name: &str| {
        let text = format!("[Desktop Entry]\nType=Application\nName={name}\nExec=true\n");
        std::fs::write(&entry, text).expect("the entry is written");
    };
    write("Scribble");
    read();
    flush();
    assert_eq!(
        names(),
        ["Notes", "Scribble"],
        "an entry installed later names it"
    );

    write("Jotter");
    read();
    flush();
    assert_eq!(
        names(),
        ["Notes", "Scribble", "Jotter"],
        "and its new name renames it"
    );
    read();
    flush();
    assert_eq!(
        names().len(),
        3,
        "reading the same entries again changes nothing"
    );
}

/// Removes its folder as it drops, so a test that fails leaves nothing behind either.
struct Removed(std::path::PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
