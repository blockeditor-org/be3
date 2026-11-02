use crate::unsafe_sites;

#[test]
fn unsafe_sites_reads_unparseable_source() {
    let sites = unsafe_sites("fn broken( {\n    // unsafe(memory)\n    unsafe { 1 };\n");

    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].line, 3);
}
