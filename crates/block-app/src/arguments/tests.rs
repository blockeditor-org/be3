use super::*;

mod desktop_runs_everywhere_and_session_only_on_linux;
mod the_embedded_server_listens_where_asked;

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}
