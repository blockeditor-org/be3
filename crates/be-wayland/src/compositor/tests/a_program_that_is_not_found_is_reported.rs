use super::*;

#[test]
fn a_program_that_is_not_found_is_reported() {
    let mut harness = Harness::new();
    let (woken, wake) = std::sync::mpsc::channel();
    harness.app.waker = Some(Waker::new(move || {
        let _ = woken.send(());
    }));
    let reported = Rc::new(RefCell::new(Vec::new()));
    let heard = reported.clone();
    harness
        .app
        .on_failure(move |problem| heard.borrow_mut().push(problem));

    harness
        .app
        .windows()
        .launch("be-wayland-test-no-such-program".to_owned());
    harness.frame(Vec::new());
    wake.recv()
        .expect("the program's exit wakes the compositor");
    harness.frame(Vec::new());

    assert_eq!(
        *reported.borrow(),
        vec!["Could not run be-wayland-test-no-such-program: the command was not found".to_owned()]
    );
    assert!(harness.app.children.is_empty());
}
