use be_lib_geometry::{Rect, pos2};

use super::*;
use crate::Keyboard;

#[test]
fn an_ime_area_reads_back_as_it_was_encoded() {
    let area = ImeArea {
        rect: Rect::from_min_max(pos2(10.5, 20.0), pos2(110.0, 44.25)),
        cursor: Rect::from_min_max(pos2(12.0, 20.0), pos2(13.0, 44.25)),
        text: None,
        keyboard: Keyboard::Shown { asks: 2 },
    };
    assert_eq!(ImeArea::decode(&area.encode()), Ok(area));
    assert!(
        ImeArea::decode(&[0xff]).is_err(),
        "bytes that are no area are an error"
    );
}
