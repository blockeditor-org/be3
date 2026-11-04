use super::*;

#[test]
fn a_programs_mime_types_name_the_extensions_it_opens() {
    let reader = entry(
        "koreader %f",
        "MimeType=application/epub+zip;application/x-cbz;image/png;",
    );
    assert_eq!(
        reader.mime_types,
        ["application/epub+zip", "application/x-cbz", "image/png"]
    );
    let globs = MimeGlobs::parse(
        "# generated\n\
         50:application/epub+zip:*.epub\n\
         50:application/vnd.comicbook+zip:*.cbz\n\
         50:image/png:*.PNG:cs\n\
         50:application/x-compressed-tar:*.tar.gz\n\
         10:text/x-readme:README*\n",
        "application/x-cbz application/vnd.comicbook+zip\n",
    );
    assert_eq!(
        globs.extensions(&reader.mime_types),
        ["cbz", "epub", "png"],
        "an alias reaches its canonical type, and extensions are lower case"
    );
    assert!(
        globs
            .extensions(&[
                "application/x-compressed-tar".to_owned(),
                "text/x-readme".to_owned()
            ])
            .is_empty(),
        "only a plain *.extension pattern names an extension"
    );

    let scratch = Scratch::new();
    scratch.write("system/mime/globs2", b"50:application/x-krita:*.kra\n");
    scratch.write("home/mime/globs2", b"50:application/x-krita:*.krz\n");
    assert_eq!(
        environment(&scratch, &["home", "system"])
            .mime_globs()
            .extensions(&["application/x-krita".to_owned()]),
        ["kra", "krz"],
        "every data directory's globs are read"
    );
}
