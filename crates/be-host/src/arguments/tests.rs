use super::*;

mod desktop_runs_everywhere_and_on_the_displays_only_on_linux;

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}
