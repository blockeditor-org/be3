use super::*;

fn send<T: Send + 'static>(value: T) -> T {
    value
}

#[test]
fn a_network_call_returns_its_typed_result_from_another_thread() {
    let pipe = pipe();
    let mut found = send(client::shelf::find(&pipe.client, "Book 7".to_owned()));
    let mut missing = client::shelf::find(&pipe.client, "Book 8".to_owned());
    assert_eq!(poll(&mut found), std::task::Poll::Pending);

    let calls = send(pipe.accept());
    let server = pipe.server.clone();
    std::thread::spawn(move || {
        for call in calls {
            let embed::shelf::Call::Find { reply, title } = call else {
                panic!("{call:?} is not a find");
            };
            let answer = match title.as_str() {
                "Book 7" => Ok(book(7)),
                _ => Err(shelf::ShelfError::Missing(title)),
            };
            reply.send(&mut server.lock(), &answer);
        }
    })
    .join()
    .unwrap();
    pipe.pump();

    assert_eq!(poll(&mut found), std::task::Poll::Ready(Ok(Ok(book(7)))));
    assert_eq!(
        poll(&mut missing),
        std::task::Poll::Ready(Ok(Err(shelf::ShelfError::Missing("Book 8".to_owned()))))
    );
}
