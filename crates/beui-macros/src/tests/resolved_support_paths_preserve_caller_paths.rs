use super::*;

#[test]
fn resolved_support_paths_preserve_caller_paths() {
    let input = quote! { { ::__beui_tree::reactive::into_child(::beui::UserValue); [::__beui_tree::reactive::Scope::new()] } };
    assert_eq!(rewrite_tree(input.clone(), "html").to_string(), quote! { { ::html::reactive::into_child(::beui::UserValue); [::html::reactive::Scope::new()] } }.to_string());
    assert_eq!(rewrite_tree(input, "crate").to_string(), quote! { { crate::reactive::into_child(::beui::UserValue); [crate::reactive::Scope::new()] } }.to_string());
}
