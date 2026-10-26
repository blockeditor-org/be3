use super::super::unsafe_counts::{count, summary};

#[test]
fn unsafe_counts_show_what_changed_in_each_category() {
    let before = count(["fn a() {\n    unsafe { b() };\n}\n"]);
    let after = count([
        "// unsafe(ffi)\nfn a() {\n    unsafe { b() };\n}\n",
        "fn c() {\n    // unsafe(env)\n    unsafe { std::env::set_var(\"A\", \"B\") };\n}\n",
    ]);
    assert_eq!(
        summary(&before, &after),
        "## Unsafe\n\nUnsafe sites by category: ffi 1 (**+1**), memory 0, env 1 (**+1**), \
         send-sync 0, untagged 0 (**-1**). Only ffi is unavoidable.\n\n"
    );
}
