use super::*;

use smithay::backend::allocator::{Format, Fourcc, Modifier};

#[test]
fn a_dmabuf_commit_waits_until_its_buffer_is_readable() {
    let mut server = server();
    server.state.enable_dmabuf(
        vec![Format {
            code: Fourcc::Argb8888,
            modifier: Modifier::Linear,
        }],
        None,
        None,
    );
    let mut client = TestClient::connect(&mut server);
    let (window, id) = client.open(&mut server);
    let (pending, writer) = rustix::pipe::pipe().expect("a pipe opens");

    client.attach_dmabuf_unsent(&window, std::os::fd::AsFd::as_fd(&pending), 40, 30);
    client.exchange(&mut server);
    assert_eq!(server.state.blocked(), 1);
    assert!(
        server.state.layers(id).is_empty(),
        "a buffer the GPU is still writing is not shown"
    );

    rustix::io::write(&writer, b"done").expect("the pipe is written");
    client.exchange(&mut server);
    assert_eq!(server.state.blocked(), 0);
    assert_eq!(
        server.state.layers(id).len(),
        1,
        "the commit lands once the buffer is readable"
    );
}
