use super::*;

const XPM: &str = r#"/* XPM */
static char * icon_xpm[] = {
/* columns rows colors chars-per-pixel */
"3 2 4 2",
"  	c None",
".. 	c #FF0000",
"++ 	s edge c #00f",
"@@ 	c gray50",
"  ..++",
"@@@@  "};
"#;

#[test]
fn an_xpm_pixmap_loads_as_an_image() {
    let scratch = Scratch::new();
    let path = scratch.write("share/pixmaps/tiny.xpm", XPM.as_bytes());
    let environment = environment(&scratch, &["share"]);
    assert_eq!(
        IconThemes::new(&environment).find("tiny", 32).as_deref(),
        Some(path.as_path())
    );
    let image = load_icon(&path, 32).expect("the pixmap decodes");
    assert_eq!((image.width(), image.height()), (3, 2));
    assert_eq!(
        image.pixels(),
        &[
            0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255, 255, 128, 128, 128, 255, 128, 128, 128, 255, 0,
            0, 0, 0,
        ]
    );

    let short = scratch.write("short.xpm", XPM.replace("\"@@@@  \"", "").as_bytes());
    assert!(load_icon(&short, 32).is_none());
}
