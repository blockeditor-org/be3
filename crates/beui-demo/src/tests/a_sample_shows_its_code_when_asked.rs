use super::*;
use crate::DemoShell;

#[test]
fn a_sample_shows_its_code_when_asked() {
    let mut test = demo(WIDE);
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, DemoShell::SOURCE),
        0,
        "a sample keeps its code folded away"
    );

    test.click("demo.code.The dock around this page");
    test.frame(Vec::new());

    assert_eq!(
        showing(test.document(), root, DemoShell::SOURCE),
        1,
        "pressing Code shows the source the sample was written with"
    );
    test.snapshot("docking_code");
}
