use super::*;

mod a_handoff_without_the_token_is_refused;
mod a_second_be_host_hands_over_to_the_first;
mod a_second_be_host_on_the_same_data_finds_the_first_running;

fn data_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("be-host-instance-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the data directory is made");
    dir
}
