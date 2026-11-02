use std::time::Duration;

use beui::native::{Context, WindowSize};
use beui::{Image, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use super::*;

const INDEX: &str = "[Icon Theme]\nName=Hicolor\nDirectories=scalable/apps\n\n\
[scalable/apps]\nSize=48\nMinSize=8\nMaxSize=512\nType=Scalable\n";

const LETTERS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
<text x="3" y="22" style="font-size:20px;font-family:Inter;fill:#c00000">X</text>
<text x="3" y="44" style="font-size:16px;font-family:Inter;fill:#000000">term</text></svg>"##;

const NOTHING: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
<rect width="16" height="16" fill="none"/></svg>"##;

const PIXMAP: &str = r#"/* XPM */
static char * pixmap_xpm[] = {
"4 4 3 1",
"  c None",
". c #2060E0",
"+ c white",
" .. ",
".++.",
".++.",
" .. "};
"#;

fn entry(name: &str, icon: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec=true\nIcon={icon}\n")
}

#[test]
fn each_programs_tile_shows_the_icon_its_entry_names() {
    let root = std::env::temp_dir().join(format!("be-host-desktop-tiles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |relative: &str, contents: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a file has a folder"))
            .expect("the folder is made");
        std::fs::write(path, contents).expect("the file is written");
    };
    write("applications/xterm.desktop", &entry("XTerm", "mini.xterm"));
    write("applications/pixmap.desktop", &entry("Pixmap", "pixmap"));
    write("applications/blank.desktop", &entry("Blank", "blank"));
    write("icons/hicolor/index.theme", INDEX);
    write("icons/hicolor/scalable/apps/mini.xterm.svg", LETTERS);
    write("icons/hicolor/scalable/apps/blank.svg", NOTHING);
    write("pixmaps/pixmap.xpm", PIXMAP);
    let source = LinuxPrograms::of(Environment {
        data_dirs: vec![root.clone()],
        ..Environment::default()
    });
    let icon = |id: &str| {
        source.icon(id, 96).map(|icon| Image::from_rgba(icon.width, icon.height, icon.rgba))
    };
    assert!(
        icon("xterm.desktop").is_some(),
        "an icon drawn only with text draws its letters"
    );
    assert!(icon("pixmap.desktop").is_some(), "an XPM pixmap is read");
    assert!(
        icon("blank.desktop").is_none(),
        "an icon that draws nothing is no icon, so its tile shows the plain glyph"
    );
    let items: Vec<PaletteItem> = source
        .programs()
        .into_iter()
        .map(|program| PaletteItem {
            image: icon(&program.id),
            key: program.id,
            title: program.name,
            glyph: Some(beui::icons::ICON_LAYOUT_GRID),
            ..PaletteItem::default()
        })
        .collect();
    let _ = std::fs::remove_dir_all(&root);

    let document = beui_host::native::build(move || {
        let items = items.clone();
        view! {
            <Root>
                <SearchList items columns=3 list_height=120.0 on_choose={Callback::new(|_| {})} />
            </Root>
        }
    });
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        WindowSize {
            size: vec2(360.0, 200.0),
            scale: 1.0,
        },
        None,
    );
    for _ in 0..3 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    stepped.snapshot("each_programs_tile_shows_the_icon_its_entry_names");
}
