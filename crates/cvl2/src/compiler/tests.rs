use super::*;
use crate::parser::{
    BinaryExpressionToken, BlockToken, ErrorStyle, IdentifierToken, WhitespaceToken,
};
use std::cell::RefCell;
use std::rc::Rc;

fn new_env() -> Env {
    Env {
        trace: Vec::new(),
        errors: Vec::new(),
        scope: Scope {
            comptime: ComptimeScopeMap::root(HashMap::new()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
        },
        fn_cache: Rc::new(PerComptimeScopeCache::new()),
        decl_cache: Rc::new(PerComptimeScopeCache::new()),
        builtin_cache: Rc::new(PerComptimeScopeCache::new()),
    }
}

fn pos_at(idx: usize) -> TokenPosition {
    TokenPosition {
        fyl: "test".to_string(),
        idx,
        lyn: 1,
        col: idx + 1,
    }
}

fn ws_node(idx: usize) -> SyntaxNode {
    SyntaxNode::Whitespace(WhitespaceToken {
        pos: pos_at(idx),
        nl: false,
    })
}

fn normal_ident(name: &str, idx: usize) -> SyntaxNode {
    SyntaxNode::Identifier(IdentifierToken {
        pos: pos_at(idx),
        str: name.to_string(),
        ident_tag: IdentifierTag::Normal,
        ident_tag_raw: String::new(),
    })
}

fn builtin_ident(name: &str, idx: usize) -> SyntaxNode {
    SyntaxNode::Identifier(IdentifierToken {
        pos: pos_at(idx),
        str: name.to_string(),
        ident_tag: IdentifierTag::Builtin,
        ident_tag_raw: "#".to_string(),
    })
}

fn op_seg(items: Vec<SyntaxNode>, idx: usize) -> SyntaxNode {
    SyntaxNode::OperatorSegment(OperatorSegmentToken {
        pos: pos_at(idx),
        items,
    })
}

fn op_node(op: &str, idx: usize) -> SyntaxNode {
    SyntaxNode::Operator(OperatorToken {
        pos: pos_at(idx),
        op: op.to_string(),
        op_tag: OpTag::None,
    })
}

fn binary_node(tag: OpTag, items: Vec<SyntaxNode>, idx: usize) -> SyntaxNode {
    SyntaxNode::BinaryExpression(Box::new(BinaryExpressionToken {
        pos: pos_at(idx),
        prec: 0,
        tag,
        items,
    }))
}

fn list_block(items: Vec<SyntaxNode>, idx: usize) -> SyntaxNode {
    SyntaxNode::Block(Box::new(BlockToken {
        pos: pos_at(idx),
        start: "(".to_string(),
        end: ")".to_string(),
        items,
        tag: BracketTag::List,
    }))
}

fn def_binding(lhs_name: &str, rhs_name: &str, idx: usize) -> SyntaxNode {
    binary_node(
        OpTag::Def,
        vec![
            op_seg(vec![normal_ident(lhs_name, idx)], idx),
            op_node("::", idx + 1),
            op_seg(vec![normal_ident(rhs_name, idx + 2)], idx + 2),
        ],
        idx,
    )
}

fn pub_binding(lhs: SyntaxNode, rhs: SyntaxNode, idx: usize) -> SyntaxNode {
    binary_node(
        OpTag::Pub,
        vec![
            op_seg(vec![lhs], idx),
            op_node(".=", idx + 1),
            op_seg(vec![rhs], idx + 2),
        ],
        idx,
    )
}

fn build_c_fn(body: &str) -> Result<String, Vec<TokenizationError>> {
    let artifact = import_file(
        "op.qxc",
        &format!(
            "#builtin.build .= () => std.Folder: [
  \"lib.c\" .= std.c.compile: [
    \"f\" .= f
  ]
]
f :: (a: std.c.int, b: std.c.int) => std.c.int: {{
{body}
}}
g :: (x: std.c.int) => std.c.int: x
std :: #builtin.std"
        ),
    )?;
    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(_, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file");
    };
    Ok(String::from_utf8(file.value.clone()).unwrap())
}

fn build_file(body: &str) -> Result<String, Vec<TokenizationError>> {
    let artifact = import_file(
        "file.qxc",
        &format!(
            "#builtin.build .= () => std.Folder: [
  \"x.txt\" .= std.File: {{
{body}
  }}
]
std :: #builtin.std"
        ),
    )?;
    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(_, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file");
    };
    Ok(String::from_utf8(file.value.clone()).unwrap())
}

fn only_error(result: Result<String, Vec<TokenizationError>>) -> String {
    let errors = result.expect_err("expected the build to fail");
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors[0].entries[0].message.clone()
}

mod add_err_pushes_error_into_env;
mod analyze_access_builtin_main_resolves_key;
mod analyze_base_builtin_resolves_to_namespace;
mod analyze_call_not_supported_call_type_errors;
mod analyze_namespace_errors_on_non_key_bind_target;
mod block_append_returns_sequential_indices;
mod get_err_includes_message_and_trace;
mod import_file_builds_c_sample;
mod import_file_builds_demo_sample;
mod import_file_builds_kw_sample;
mod import_file_builds_loop_sample;
mod import_file_builds_types_sample;
mod import_file_c_if_requires_c_target;
mod import_file_compare_operator_takes_rhs_type_from_lhs;
mod import_file_kw_bool_values_resolve_from_slot;
mod import_file_kw_else_must_follow_if_on_same_line;
mod import_file_kw_if_condition_must_be_kw_bool;
mod import_file_kw_if_requires_build_target;
mod import_file_kw_int_arithmetic_errors_on_overflow;
mod import_file_loop_and_mut_errors;
mod import_file_operator_errors_without_slot_or_lhs;
mod import_file_operator_falls_back_to_lhs_in_unknown_slot;
mod import_file_operator_name_must_be_an_operator;
mod import_file_user_type_errors;
mod import_file_written_calls_match_calls;
mod import_file_written_literals_match_literals;
mod import_file_written_operators_match_desugared_operators;
mod ns_key_distinguishes_str_and_sym_variants;
mod per_comptime_scope_cache_keys_on_target;
mod read_binary2_extracts_matching_triplet;
mod read_binary2_returns_none_for_empty_input;
mod read_binary2_returns_none_for_non_matching_tag;
mod read_binary2_wrong_length_errors;
mod read_binary_extracts_segments_from_matching_binary;
mod read_binary_returns_empty_for_empty_input;
mod read_binary_unexpected_token_errors;
mod read_binary_wraps_non_matching_single_segment;
mod read_container_collects_bindings_and_lines;
mod read_container_duplicate_binding_adds_error;
mod read_destructure_empty_input_reports_error;
mod read_destructure_extra_item_reports_error;
mod read_destructure_list_of_items;
mod read_destructure_single_item;
mod read_destructure_unsupported_kind_errors;
mod symbol_new_produces_unique_symbols;
mod throw_err_wraps_get_err_result;
mod trim_ws_filters_whitespace_nodes;
