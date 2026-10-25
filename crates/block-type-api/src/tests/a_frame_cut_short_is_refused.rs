use super::*;

#[test]
fn a_frame_cut_short_is_refused() {
    let frame = encode_frame(&Event::Tick { now: 1 });
    assert!(matches!(
        decode_frame::<Event>(&frame[..frame.len() - 1]),
        Err(FrameError::Truncated { .. })
    ));
    assert!(matches!(
        decode_frame::<Event>(&frame[..2]),
        Err(FrameError::Truncated { .. })
    ));
}
