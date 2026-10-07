use super::*;

#[test]
fn a_wake_from_another_thread_runs_another_frame() {
    let log = Log::default();
    let app = Recorder {
        log: Rc::clone(&log),
        frames: 0,
        on_frame: |context: &Context, frame| {
            if frame == 2 {
                context.close_window();
            }
        },
    };

    run_headless(app).expect("the headless run ends without an error");

    assert_eq!(
        *log.borrow(),
        ["setup", "frame 1", "frame 2", "exiting", "dropped"],
        "a frame that asks for no repaint waits for the thread's wake, which runs the second",
    );
}
