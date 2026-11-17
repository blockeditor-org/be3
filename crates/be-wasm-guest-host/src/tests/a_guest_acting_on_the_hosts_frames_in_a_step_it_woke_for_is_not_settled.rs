use super::*;

/// A guest that takes the host's frames in one step and acts on them in the next, which it wakes
/// itself for (an answer it reads and then asks a frame for), has not settled until that next
/// step is done: `settled` must not answer between the two.
#[test]
fn a_guest_acting_on_the_hosts_frames_in_a_step_it_woke_for_is_not_settled() {
    let (mut worker, _) = echo();
    assert_eq!(worker.queue(vec![be_wasm_guest_echo::WAKE.to_vec()]), None);
    assert!(!worker.settled(), "the step taking the frames is in flight");
    let received = polled(&mut worker, 1).unwrap();
    assert!(!received.contains(&be_wasm_guest_echo::WOKEN.to_vec()));
    assert!(
        !worker.settled(),
        "the step the guest woke itself for carries on the frames' work"
    );
    let received = polled(&mut worker, 1).unwrap();
    assert_eq!(received, vec![be_wasm_guest_echo::WOKEN.to_vec()]);
    assert!(worker.settled());
}
