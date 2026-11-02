pub fn memory_type(types: u32, preferred: impl Fn(u32) -> bool) -> Option<u32> {
    let allowed = (0..32).filter(|index| types & (1 << index) != 0);
    allowed
        .clone()
        .find(|&index| preferred(index))
        .or_else(|| allowed.min())
}
