use super::*;

#[test]
fn a_result_longer_than_its_room_waits_to_be_taken() {
    let mut guest = Guest::new();
    let message = "a shader did not compile ".repeat(8);
    guest.gpu.report(message.clone());
    let length = glue::take_error(&mut guest, 64, 4);
    assert!(length > 4);
    assert_eq!(guest.memory[64..68], [0; 4]);
    glue::returned(&mut guest, 128);
    let bytes = &guest.memory[128..128 + length as usize];
    assert_eq!(be_ops::decode::<Option<String>>(bytes), Some(Some(message)));
    assert!(guest.refused.is_empty());
}
