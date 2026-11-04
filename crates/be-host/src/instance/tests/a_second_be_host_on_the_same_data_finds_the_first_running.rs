use super::*;

#[test]
fn a_second_be_host_on_the_same_data_finds_the_first_running() {
    let dir = data_dir("running");
    let Ok(Claim::First(first)) = claim(&dir) else {
        panic!("the first be-host claims its data");
    };
    let Ok(Claim::Running(running)) = claim(&dir) else {
        panic!("a second be-host on the same data finds the first running");
    };
    assert_eq!(running.data_dir(), dir);
    let Ok(Claim::First(_)) = claim(&data_dir("running-elsewhere")) else {
        panic!("a be-host on other data is not stopped by the first");
    };
    drop(first);
    let Ok(Claim::First(_)) = claim(&dir) else {
        panic!("the data is free once the first be-host is gone");
    };
}
