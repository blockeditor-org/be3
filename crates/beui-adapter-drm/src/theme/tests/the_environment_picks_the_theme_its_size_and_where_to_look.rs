use super::*;

#[test]
fn the_environment_picks_the_theme_its_size_and_where_to_look() {
    let unset = Environment {
        home: Some("/home/me".to_owned()),
        ..Environment::default()
    };
    assert_eq!(
        Theme::of(&unset),
        Theme::new(
            "default",
            24,
            [
                "/home/me/.local/share/icons",
                "/home/me/.icons",
                "/usr/local/share/icons",
                "/usr/share/icons",
                "/usr/share/pixmaps",
                "/usr/share/cursors/xorg-x11",
            ]
            .map(PathBuf::from)
            .to_vec(),
        )
    );

    let xdg = Environment {
        theme: Some("Adwaita".to_owned()),
        size: Some("48".to_owned()),
        home: Some("/home/me".to_owned()),
        xdg_data_home: Some("~/data".to_owned()),
        xdg_data_dirs: Some("/opt/share::/usr/share".to_owned()),
        ..Environment::default()
    };
    assert_eq!(
        Theme::of(&xdg),
        Theme::new(
            "Adwaita",
            48,
            [
                "/home/me/data/icons",
                "/home/me/.icons",
                "/opt/share/icons",
                "/usr/share/icons",
                "/usr/share/pixmaps",
                "/usr/share/cursors/xorg-x11",
            ]
            .map(PathBuf::from)
            .to_vec(),
        )
    );

    let listed = Environment {
        size: Some("huge".to_owned()),
        home: Some("/home/me".to_owned()),
        xcursor_path: Some("~/.cursors:/srv/icons".to_owned()),
        xdg_data_dirs: Some("/ignored".to_owned()),
        ..Environment::default()
    };
    assert_eq!(
        Theme::of(&listed),
        Theme::new(
            "default",
            24,
            ["/home/me/.cursors", "/srv/icons"]
                .map(PathBuf::from)
                .to_vec(),
        ),
        "XCURSOR_PATH replaces the search path, and a size that is not a number is ignored"
    );
}
