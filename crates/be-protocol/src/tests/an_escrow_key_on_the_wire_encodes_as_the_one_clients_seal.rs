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
    let wire = types::EscrowKey {
        id: frozen.id,
        epoch: frozen.epoch,
        public: frozen.public,
        signing: frozen.signing,
        signature: frozen.signature.clone(),
    };
    assert_eq!(postcard::to_stdvec(&wire).unwrap(), frozen.encode());
}
