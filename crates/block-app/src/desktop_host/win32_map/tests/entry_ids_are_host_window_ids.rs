use super::*;

#[test]
fn entry_ids_are_host_window_ids() {
    assert_eq!(window_id(EntryId(12)), HostWindowId(12));
    assert_eq!(entry_id(HostWindowId(12)), EntryId(12));
}
