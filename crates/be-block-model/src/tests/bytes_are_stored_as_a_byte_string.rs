use crate::Stored;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Payload {
    #[serde(with = "crate::bytes")]
    data: Vec<u8>,
    action: crate::Bytes,
}

#[test]
fn bytes_are_stored_as_a_byte_string() {
    let payload = Payload {
        data: vec![1, 200, 3],
        action: crate::Bytes(vec![4, 5]),
    };

    let stored: Stored = crate::from_cbor(&crate::to_cbor(&payload)).expect("it decodes");

    let Stored::Map(fields) = stored else {
        panic!("a struct is a map: {stored:?}");
    };
    assert_eq!(
        fields
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>(),
        vec![Stored::Bytes(vec![1, 200, 3]), Stored::Bytes(vec![4, 5])]
    );
    assert_eq!(
        crate::from_cbor::<Payload>(&crate::to_cbor(&payload)),
        Some(payload)
    );
}
