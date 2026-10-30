use super::*;

#[test]
fn an_archive_is_rewritten_with_its_symbol_table_moved() {
    let object = reaching_object("ELSEWHERE", 0);
    let bytes = archive(&[("reach.o/", &object), ("lib.rmeta/", b"odd")], "reach");
    let foreign = HashSet::from(["ELSEWHERE".to_owned()]);
    let rewritten = archive::rewritten(&bytes, |member| tls::through_got(member, &foreign))
        .unwrap()
        .unwrap();
    let members = archive::members(&rewritten).unwrap();
    assert_eq!(members.len(), 3);
    let at = u32::from_be_bytes(members[0].data[4..8].try_into().unwrap()) as usize;
    assert_eq!(at, members[1].start);
    assert_eq!(
        module::imports(members[1].data).unwrap()[1].name,
        "ELSEWHERE"
    );
    assert_eq!(members[2].data, b"odd");
    let untouched = archive::rewritten(&bytes, |_| Ok(None)).unwrap();
    assert_eq!(untouched, None);
}
