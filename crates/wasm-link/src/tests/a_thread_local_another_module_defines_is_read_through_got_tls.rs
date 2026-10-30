use super::*;

#[test]
fn a_thread_local_another_module_defines_is_read_through_got_tls() {
    let object = reaching_object("ELSEWHERE", 0);
    assert_eq!(
        tls::scan(&object).unwrap().reached,
        HashSet::from(["ELSEWHERE".to_owned()])
    );
    let foreign = HashSet::from(["ELSEWHERE".to_owned()]);
    let rewritten = tls::through_got(&object, &foreign).unwrap().unwrap();
    let imports = module::imports(&rewritten).unwrap();
    assert_eq!(
        imports
            .iter()
            .map(|import| (import.module.as_str(), import.name.as_str()))
            .collect::<Vec<_>>(),
        [("env", "__tls_base"), ("GOT.tls", "ELSEWHERE")]
    );
    let code = module::sections(&rewritten)
        .unwrap()
        .into_iter()
        .find(|section| section.id == 10)
        .unwrap();
    assert_eq!(code.contents[3], 0x23);
    assert_eq!(code.contents[9], 0x23);
    let found = tls::scan(&rewritten).unwrap();
    assert!(found.reached.is_empty());
    let relocations = module::custom(&rewritten, "reloc.CODE").unwrap().unwrap();
    assert_eq!(relocations, [3, 2, 7, 4, 1, 7, 10, 2]);
}
