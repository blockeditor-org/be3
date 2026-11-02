use super::*;

#[test]
fn a_thread_local_defined_in_the_link_stays_local() {
    let definer = object::thread_local_segment(
        ".tbss.here",
        4,
        2,
        &[object::Symbol {
            name: "HERE",
            offset: 0,
            size: 4,
        }],
        0,
    );
    assert_eq!(
        tls::scan(&definer).unwrap().defined,
        HashSet::from(["HERE".to_owned()])
    );
    let object = reaching_object("HERE", 0);
    let foreign = HashSet::from(["ELSEWHERE".to_owned()]);
    assert_eq!(tls::through_got(&object, &foreign).unwrap(), None);
}
