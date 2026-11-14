use std::io::Write as _;

use super::*;

#[test]
fn a_painting_is_unchanged_however_it_was_compressed() {
    let red = triangle([255, 0, 0, 255]);
    let accepted = red.encode().unwrap();
    // The magic and the version, then the body deflated less hard.
    let mut fast = accepted[..12].to_vec();
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&red.body().unwrap()).unwrap();
    fast.extend(encoder.finish().unwrap());
    assert_ne!(fast, accepted);
    assert_eq!(crate::changed(&accepted, &fast), None);

    let green = triangle([0, 255, 0, 255]).encode().unwrap();
    assert!(crate::changed(&accepted, &green).is_some());
    assert_eq!(
        crate::changed(b"", &accepted).unwrap(),
        "the accepted painting is unreadable: not a paint snapshot"
    );
}
