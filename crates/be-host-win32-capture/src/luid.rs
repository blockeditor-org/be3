pub type Luid = [u8; 8];

pub fn luid(low: u32, high: i32) -> Luid {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&low.to_le_bytes());
    bytes[4..].copy_from_slice(&high.to_le_bytes());
    bytes
}

pub fn adapter_matching(wanted: Luid, adapters: impl IntoIterator<Item = Luid>) -> Option<usize> {
    adapters.into_iter().position(|adapter| adapter == wanted)
}
