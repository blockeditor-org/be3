use super::*;

use std::io::Write as _;
use std::os::fd::AsFd;

use crate::test_client::{pattern, read};

#[test]
fn a_dmabuf_window_samples_the_clients_pixels() {
    let (mut harness, device, queue) = Harness::with_gpu();
    let (window, id) = harness.open();
    let pixels = pattern(64, 16);
    let fd =
        rustix::fs::memfd_create("dmabuf", rustix::fs::MemfdFlags::CLOEXEC).expect("a memfd opens");
    let mut file = std::fs::File::from(fd);
    file.write_all(&pixels).expect("the pixels are written");

    harness
        .client
        .attach_dmabuf_unsent(&window, file.as_fd(), 64, 16);
    harness.settle();

    let surface = harness.app.server().state.surface(id).unwrap();
    let current = harness
        .app
        .textures
        .borrow()
        .get(&surface)
        .expect("the window has a texture");
    assert!(
        current.buffer.is_some(),
        "the window samples the client's buffer rather than a copy"
    );
    assert_eq!(read(&device, &queue, current.texture.texture()), pixels);
}
