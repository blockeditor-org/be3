use super::*;

#[test]
fn an_app_that_closes_its_window_exits_before_it_is_dropped() {
    let log = Log::default();
    let app = Recorder {
        log: Rc::clone(&log),
        frames: 0,
        on_frame: |context: &Context, frame| match frame {
            3 => context.close_window(),
            _ => context.request_repaint(),
        },
    };

    run_headless(app).expect("the headless run ends without an error");

    assert_eq!(
        *log.borrow(),
        [
            "setup", "frame 1", "frame 2", "frame 3", "exiting", "dropped"
        ],
    );
}
