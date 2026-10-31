use super::*;

#[test]
fn an_escrow_key_on_the_wire_encodes_as_the_one_clients_seal() {
    let frozen = EscrowKey {
        id: Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
        epoch: 300,
        public: [1; 32],
        signing: [2; 32],
        signature: vec![3; 64],
    };
    let sealed = [
        vec![16],
        frozen.id.as_bytes().to_vec(),
        vec![0xac, 0x02],
        vec![1; 32],
        vec![2; 32],
        vec![64],
        vec![3; 64],
    ]
    .concat();
    assert_eq!(frozen.encode(), sealed);
    assert_eq!(EscrowKey::decode(&sealed), Some(frozen.clone()));
    let opened = OrganizationOpened {
        organization: Uuid::nil(),
        role: OrganizationRole::Editor,
        kind: OrganizationKind::Company,
        escrows: vec![frozen],
        pinned: Vec::new(),
        server: ServerAccount {
            account: Uuid::nil(),
            public: [0; 32],
        },
    };
    let wire = postcard::to_stdvec(&opened).unwrap();
    assert!(
        wire.windows(sealed.len()).any(|window| window == sealed),
        "the escrow chain travels in the bytes clients seal"
    );
}
