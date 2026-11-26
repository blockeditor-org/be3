use super::*;

#[test]
fn decoding_allocates_within_a_budget() {
    // A list of four billion units, in nine bytes.
    let mut units = vec![Wire::Empty as u8];
    units.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(kind(decode::<Vec<()>>(&units)), ErrorKind::OverBudget);
    // A list claiming more items than its bytes could hold is refused before allocating.
    let mut many = vec![Wire::Message as u8];
    many.extend_from_slice(&1000u32.to_le_bytes());
    assert_eq!(kind(decode::<Vec<PersonV1>>(&many)), ErrorKind::BadList);
    // The budget is the caller's to give.
    let list = bytes(&vec![(); 100]);
    assert_eq!(decode_with_budget::<Vec<()>>(&list, 100), Ok(vec![(); 100]));
    assert_eq!(
        kind(decode_with_budget::<Vec<()>>(&list, 99)),
        ErrorKind::OverBudget
    );
}
