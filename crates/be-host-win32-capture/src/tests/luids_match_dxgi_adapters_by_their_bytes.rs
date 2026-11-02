use super::*;

#[test]
fn luids_match_dxgi_adapters_by_their_bytes() {
    assert_eq!(luid(0x0403_0201, 0x0807_0605), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(luid(1, -1), [1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    let adapters = [luid(7, 0), luid(0x1234, 0), luid(0x1234, 1)];
    assert_eq!(adapter_matching(luid(0x1234, 1), adapters), Some(2));
    assert_eq!(adapter_matching(luid(9, 0), adapters), None);
}
