use be_sdk_root_testing::{LATER, ROOT_SURFACE};

use super::*;

#[test]
fn a_roots_document_draws_again_when_the_frame_it_asked_for_comes() {
    let mut root = embedding();
    let first = root
        .frame(
            FrameRequest {
                time: 1,
                wall_clock: 0,
            },
            Instant::now() + PATIENT,
        )
        .expect("the root draws");
    let drawn = root
        .own_surface(ROOT_SURFACE)
        .expect("the root drew its own surface")
        .generation;
    let before = glyphs(&own_painting(&mut root, 1).frame.primitives).len();
    let later = u64::try_from(LATER.as_nanos()).expect("in range") + 1;
    let next = first
        .next
        .expect("the root wants a frame for its document's timer");
    assert!(
        next <= later,
        "the root asks for a frame by the time its document's timer is due ({later}), not {next}"
    );

    // The document's timer runs on the host's frame clock, which stands still between these:
    // a frame for the same time draws nothing new, however long the test took.
    assert_eq!(
        glyphs(&own_painting(&mut root, 1).frame.primitives).len(),
        before,
        "the document reads the host's frame clock, not a clock of its own"
    );

    root.frame(
        FrameRequest {
            time: later,
            wall_clock: 0,
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws");
    let redrawn = root
        .own_surface(ROOT_SURFACE)
        .expect("the root drew its own surface")
        .generation;
    assert_ne!(
        redrawn, drawn,
        "the frame its document asked for comes, and it draws and presents its surface again"
    );
    let after = glyphs(&own_painting(&mut root, later).frame.primitives).len();
    assert!(
        after > before,
        "the document shows its longer line once its timer fired: {before} glyphs, then {after}"
    );
}
