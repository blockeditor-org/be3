use super::*;

const HICOLOR: &str = "[Icon Theme]
Name=Hicolor
Directories=16x16/apps,48x48/apps,64x64/apps,scalable/apps

[16x16/apps]
Size=16
Type=Threshold

[48x48/apps]
Size=48
Type=Fixed

[64x64/apps]
Size=64
Type=Fixed

[scalable/apps]
Size=128
MinSize=8
MaxSize=512
Type=Scalable
";

const MINE: &str = "[Icon Theme]
Name=Mine
Inherits=hicolor
Directories=apps/48

[apps/48]
Size=48
";

#[test]
fn an_icon_is_found_at_the_size_closest_to_the_one_asked_for() {
    let scratch = Scratch::new();
    scratch.write("data/icons/hicolor/index.theme", HICOLOR.as_bytes());
    for icon in [
        "16x16/apps/editor.png",
        "48x48/apps/editor.png",
        "64x64/apps/editor.png",
        "scalable/apps/editor.svg",
        "16x16/apps/tiny.png",
        "48x48/apps/vector.png",
        "scalable/apps/vector.svg",
    ] {
        scratch.write(&format!("data/icons/hicolor/{icon}"), b"");
    }
    scratch.write("data/pixmaps/old.png", b"");
    scratch.write("home/.icons/Mine/index.theme", MINE.as_bytes());
    scratch.write("home/.icons/Mine/apps/48/editor.png", b"");
    scratch.write(
        "home/.config/gtk-3.0/settings.ini",
        b"[Settings]\ngtk-icon-theme-name=Mine\n",
    );

    let themes = IconThemes::new(&environment(&scratch, &["data"]));
    assert!(
        ends_with(
            themes.find("editor", 64),
            "home/.icons/Mine/apps/48/editor.png"
        ),
        "the chosen theme is looked in before the ones it inherits"
    );
    assert!(ends_with(
        themes.find("vector", 64),
        "hicolor/scalable/apps/vector.svg"
    ));
    assert!(
        ends_with(themes.find("tiny", 64), "hicolor/16x16/apps/tiny.png"),
        "a far size is better than none"
    );
    assert!(ends_with(themes.find("old", 64), "data/pixmaps/old.png"));
    assert!(themes.find("missing", 64).is_none());

    let plain = IconThemes::new(&Environment {
        home: None,
        ..environment(&scratch, &["data"])
    });
    assert!(ends_with(
        plain.find("editor", 64),
        "hicolor/64x64/apps/editor.png"
    ));
    assert!(ends_with(
        plain.find("editor", 48),
        "hicolor/48x48/apps/editor.png"
    ));
    let absolute = scratch.path("data/pixmaps/old.png");
    assert_eq!(
        plain.find(&absolute.to_string_lossy(), 64),
        Some(absolute.clone()),
        "an absolute icon is used as it is"
    );
}
