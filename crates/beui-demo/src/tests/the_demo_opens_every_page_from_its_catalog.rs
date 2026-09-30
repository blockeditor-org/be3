use super::*;

#[test]
fn the_demo_opens_every_page_from_its_catalog() {
    let mut test = demo(WIDE);
    for page in PAGES {
        open(&mut test, page);
        let root = test.document().root().expect("the demo built a root");
        assert!(
            showing(test.document(), root, page.title()) >= 2,
            "opening {} gives it a tab beside the catalog that named it",
            page.title()
        );
    }
}
