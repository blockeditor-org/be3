use crate::{Renderer, Screen, Terminal};

mod a_paste_is_bracketed_when_the_program_asks;
mod answers_queries_through_the_reply_callback;
mod arrow_keys_follow_the_application_cursor_mode;
mod renders_written_text;
mod reports_cursor_position;
mod resolves_styles_and_colors;
mod rows_carry_their_soft_wrap;
mod rows_that_scroll_away_are_read_from_history;
mod struct_layouts_match_the_library;

fn render(cols: u16, rows: u16, data: &str) -> Screen {
    let mut terminal = Terminal::new(cols, rows, 100).unwrap();
    terminal.write(data.as_bytes());
    let mut renderer = Renderer::new().unwrap();
    renderer.update(&mut terminal).unwrap().clone()
}
