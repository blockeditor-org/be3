use super::*;

#[test]
fn an_escrow_key_on_the_wire_encodes_as_the_one_clients_seal() {
    let frozen = EscrowKey {
        id: Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
        epoch: 300,
        public: [1; 32],
        signing: [2; 32],
        head: [5; 32],
        maker: Uuid::from_u128(0x4444),
        signature: vec![3; 64],
    };
    let sealed = [
        vec![16],
        frozen.id.as_bytes().to_vec(),
        vec![0xac, 0x02],
        vec![1; 32],
        vec![2; 32],
        vec![5; 32],
        vec![16],
        Uuid::from_u128(0x4444).as_bytes().to_vec(),
        vec![64],
        vec![3; 64],
    ]
    .concat();
    assert_eq!(frozen.encode(), sealed);
    assert_eq!(EscrowKey::decode(&sealed), Some(frozen.clone()));
    let listed = Page::Organizations(vec![Organization {
        id: Uuid::nil(),
        name: String::new(),
        kind: OrganizationKind::Company,
        role: OrganizationRole::Editor,
        escrows: vec![frozen],
    }]);
    let wire = postcard::to_stdvec(&listed).unwrap();
    assert!(
        wire.windows(sealed.len()).any(|window| window == sealed),
        "the escrow chain travels in the bytes clients seal"
    );
}
