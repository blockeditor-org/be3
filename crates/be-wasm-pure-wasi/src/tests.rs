use super::*;

mod a_guest_that_exits_stops_with_its_status;
mod a_zero_seed_draws_chacha20s_own_stream;
mod what_a_guest_writes_to_standard_error_reaches_the_log;

/// A guest's memory of `size` bytes.
pub(super) struct Bytes(pub(super) Vec<u8>);

impl Memory for Bytes {
    fn size(&self) -> usize {
        self.0.len()
    }

    fn read(&self, at: u32, length: u32) -> Result<Vec<u8>, String> {
        self.0
            .get(at as usize..at as usize + length as usize)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "outside the memory".to_owned())
    }

    fn write(&mut self, at: u32, bytes: &[u8]) -> Result<(), String> {
        self.0
            .get_mut(at as usize..at as usize + bytes.len())
            .ok_or_else(|| "outside the memory".to_owned())?
            .copy_from_slice(bytes);
        Ok(())
    }
}
