use super::*;

#[test]
fn a_mangled_symbol_is_shortened_and_others_are_kept() {
    let mangled = "_RNvMs_NtCs4fqI2P2rA04_8reactive6memoNtB4_4Memo3get";
    let short = short_symbol(mangled);
    assert_eq!(short.len(), 17);
    assert!(short.starts_with(SHORT_SYMBOL_PREFIX));
    assert_eq!(short, short_symbol(mangled));
    assert_ne!(
        short,
        short_symbol("_RNvMs_NtCs4fqI2P2rA04_8reactive6memoNtB4_4Memo3set")
    );
    assert!(
        short_symbol("anon.f0eed36672583d82a5d5643640160405.1.llvm.4702199943595589468")
            .starts_with('#')
    );
    for kept in [
        APPLY_DATA_RELOCS,
        "_initialize",
        "FT_Load_Glyph",
        "hb_buffer_create_with_a_long_name",
        "_ZN1a1bE",
    ] {
        assert_eq!(short_symbol(kept), kept);
    }
}
