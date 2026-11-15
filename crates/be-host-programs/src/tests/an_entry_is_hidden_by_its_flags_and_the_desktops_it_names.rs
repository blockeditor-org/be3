use super::*;

#[test]
fn an_entry_is_hidden_by_its_flags_and_the_desktops_it_names() {
    let desktops = vec!["be3".to_owned()];
    assert!(entry("app", "").shown_in(&desktops));
    assert!(!entry("app", "NoDisplay=true").shown_in(&desktops));
    assert!(!entry("app", "Hidden=true").shown_in(&desktops));
    assert!(!entry("app", "OnlyShowIn=GNOME;KDE;").shown_in(&desktops));
    assert!(entry("app", "OnlyShowIn=GNOME;be3;").shown_in(&desktops));
    assert!(!entry("app", "NotShowIn=be3;").shown_in(&desktops));
    assert!(entry("app", "NotShowIn=KDE;").shown_in(&desktops));
    assert!(
        !entry("", "").shown_in(&desktops),
        "an entry runs something"
    );
    let link = DesktopEntry::parse(
        "link.desktop",
        "/link.desktop",
        "[Desktop Entry]\nType=Link\nName=Site\nURL=https://example.com\n",
        &[],
    )
    .expect("the link parses");
    assert!(!link.shown_in(&desktops), "only applications are listed");
}
