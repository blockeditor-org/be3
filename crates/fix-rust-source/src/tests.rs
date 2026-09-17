use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("fix-rust-source-{}-{sequence}", std::process::id()))
}

fn formatted(source: &str) -> String {
    String::from_utf8(crate::format_views(source.as_bytes()).expect("source parses")).unwrap()
}

mod fix_repository_check_reports_without_changes;
mod fix_repository_does_not_rewrite_compliant_repository;
mod fix_repository_enforces_rust_layout;
mod fix_repository_formats_views;
mod fix_repository_ignores_sources_outside_crates;
mod fix_repository_removes_test_path_attributes;
mod fix_repository_skips_hidden_directories;
mod format_views_always_breaks_children_onto_their_own_lines;
mod format_views_breaks_a_nested_view_inside_an_expression;
mod format_views_breaks_a_tag_that_does_not_fit;
mod format_views_collapses_an_element_whose_children_are_expressions;
mod format_views_keeps_braces_around_a_value_it_cannot_write_bare;
mod format_views_keeps_the_macro_body_on_its_own_lines;
mod format_views_leaves_a_view_it_cannot_parse_alone;
mod format_views_preserves_a_multi_line_string_literal;
mod format_views_shifts_a_multi_line_expression_to_its_new_indent;
mod format_views_unwraps_a_braced_attribute_value_it_can_write_bare;
mod strip_comments;
