use super::*;

#[test]
fn a_typed_code_is_normalized() {
    let code = pairing_code();
    assert_eq!(code.len(), CODE_LEN);
    assert_eq!(normalize_code(&code), Some(code.clone()));
    let typed = format!(
        " {}-{} ",
        code[..4].to_lowercase(),
        code[4..].to_lowercase()
    );
    assert_eq!(normalize_code(&typed), Some(code));
    assert_eq!(normalize_code("o1l2-3456"), Some("01123456".to_owned()));
    assert_eq!(normalize_code("1234567"), None);
    assert_eq!(normalize_code("1234567U"), None);
}
