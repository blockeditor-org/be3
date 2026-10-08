use super::*;
use audio::{level, stepped};

const FULL: u32 = 0x10000;
const STEP: u32 = 3277;

#[test]
fn volume_steps_stop_at_full_and_at_silence() {
    assert_eq!(stepped(&[0, 0], VOLUME_STEP), vec![STEP, STEP]);
    assert_eq!(
        stepped(&[FULL / 2, FULL / 2], VOLUME_STEP),
        vec![FULL / 2 + STEP; 2]
    );
    assert_eq!(
        stepped(&[FULL - 100, FULL - 100], VOLUME_STEP),
        vec![FULL, FULL],
        "a step up stops at full volume"
    );
    assert_eq!(
        stepped(&[FULL + 5000], VOLUME_STEP),
        vec![FULL + 5000],
        "a step up leaves a volume already past full alone"
    );
    assert_eq!(
        stepped(&[FULL + 5000], -VOLUME_STEP),
        vec![FULL + 5000 - STEP]
    );
    assert_eq!(
        stepped(&[1000, 2000], -VOLUME_STEP),
        vec![0, 0],
        "a step down stops at silence"
    );
    assert_eq!(
        stepped(&[FULL / 4, FULL / 2], VOLUME_STEP),
        vec![(FULL / 2 + STEP) / 2, FULL / 2 + STEP],
        "the balance between channels is kept"
    );
    assert_eq!(
        level(&[FULL / 2, FULL / 4], true),
        Level {
            volume: 0.5,
            muted: true,
        }
    );
}
