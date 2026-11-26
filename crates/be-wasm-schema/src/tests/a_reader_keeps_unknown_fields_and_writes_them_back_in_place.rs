use super::*;

#[test]
fn a_reader_keeps_unknown_fields_and_writes_them_back_in_place() {
    let bytes = encode(&person_v2());
    let kept: PersonKeeping = decode(&bytes).unwrap();
    assert_eq!(kept.rest.ids().collect::<Vec<_>>(), [3, 4]);
    assert_eq!(encode(&kept), bytes);
    assert_eq!(decode::<PersonV2>(&encode(&kept)), Ok(person_v2()));
}
