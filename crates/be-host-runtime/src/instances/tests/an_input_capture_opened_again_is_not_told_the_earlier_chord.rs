use super::*;

use be_sdk_api::{
    CapturedChord, CapturedTrigger, DisplaysWatch, InputCapture, InputCaptureWatch, Watched,
};

#[test]
fn an_input_capture_opened_again_is_not_told_the_earlier_chord() {
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(watch::<InputCaptureWatch>(&mut instances));
    let caught = InputCapture {
        held: Vec::new(),
        chord: Some(CapturedChord {
            held: vec![29],
            trigger: CapturedTrigger::Source(46),
        }),
    };
    assert!(publish::<InputCaptureWatch>(&mut instances, &caught));
    assert_eq!(
        watched_sent::<InputCaptureWatch>(&instances.next_screens(PASS).opened),
        vec![caught.clone()]
    );

    instances.unwatch_published(INSTANCE, InputCaptureWatch::OPEN);
    assert!(
        !instances.watches_published(InputCaptureWatch::OPEN),
        "a closed watch is not watched"
    );
    assert!(watch::<InputCaptureWatch>(&mut instances));
    assert!(
        watched_sent::<InputCaptureWatch>(&instances.next_screens(PASS).opened).is_empty(),
        "a new capture starts from nothing"
    );
    let waiting = InputCapture {
        held: vec![29],
        chord: None,
    };
    assert!(publish::<InputCaptureWatch>(&mut instances, &waiting));
    assert_eq!(
        watched_sent::<InputCaptureWatch>(&instances.next_screens(PASS).opened),
        vec![waiting]
    );

    assert!(!publish::<DisplaysWatch>(&mut instances, &Vec::new()));
    assert!(watch::<DisplaysWatch>(&mut instances));
    instances.unwatch_published(INSTANCE, DisplaysWatch::OPEN);
    assert!(watch::<DisplaysWatch>(&mut instances));
    assert_eq!(
        watched_sent::<DisplaysWatch>(&instances.next_screens(PASS).opened),
        vec![Vec::new()],
        "any other watch opened again is told what is published"
    );
}
