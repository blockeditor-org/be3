use super::*;

#[test]
fn a_second_be_host_hands_over_to_the_first() {
    let dir = data_dir("hands-over");
    let Ok(Claim::First(first)) = claim(&dir) else {
        panic!("the first be-host claims its data");
    };
    let (handed, heard) = std::sync::mpsc::channel();
    first.serve(move |handoff| {
        let _ = handed.send(handoff);
    });
    let Ok(Claim::Running(running)) = claim(&dir) else {
        panic!("a second be-host finds the first running");
    };
    running
        .hand_over(Handoff {
            dev_workspace: true,
        })
        .expect("the first be-host answers");
    assert_eq!(
        heard.recv().expect("the first be-host heard the second"),
        Handoff {
            dev_workspace: true
        }
    );
    running
        .hand_over(Handoff::default())
        .expect("the first be-host answers every second be-host");
    assert_eq!(
        heard.recv().expect("the first be-host heard the second again"),
        Handoff::default()
    );
}
