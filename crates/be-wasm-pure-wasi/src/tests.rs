use super::*;

mod a_guest_has_no_arguments_environment_or_clock;
mod a_guest_that_exits_stops_with_its_status_and_logs_what_it_left_unfinished;
mod a_guest_writing_to_a_file_is_told_it_has_none;
mod a_seed_is_chacha20s_key_word_by_word;
mod a_zero_seed_draws_chacha20s_blocks_across_their_boundaries;
mod randomness_outside_the_memory_stops_the_guest;
mod what_a_guest_writes_reaches_the_log_a_line_at_a_time;

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

/// The stream `seed` draws, `lengths` bytes at a time, each draw at the start of the memory.
pub(super) fn drawn(seed: [u8; 32], lengths: &[u64]) -> Vec<u8> {
    let mut wasi = Wasi::new(seed);
    let mut memory = Bytes(vec![0; 256]);
    let mut drawn = Vec::new();
    for &length in lengths {
        let answered = wasi.call("random_get", &[0, length], &mut memory, &mut |_| {});
        assert_eq!(answered, Ok(SUCCESS));
        drawn.extend_from_slice(&memory.0[..length as usize]);
    }
    drawn
}

/// Writes `text` to the file descriptor `fd` from one iovec, answering the errno and what the
/// guest was told it wrote.
pub(super) fn write(wasi: &mut Wasi, fd: u64, text: &[u8], log: &mut dyn FnMut(&str)) -> (i32, u32) {
    let mut memory = Bytes(vec![0; 64]);
    memory.write(0, &16u32.to_le_bytes()).unwrap();
    memory.write(4, &(text.len() as u32).to_le_bytes()).unwrap();
    memory.write(16, text).unwrap();
    let errno = wasi.call("fd_write", &[fd, 0, 1, 8], &mut memory, log).unwrap();
    let written = u32::from_le_bytes(memory.read(8, 4).unwrap().try_into().unwrap());
    (errno, written)
}
