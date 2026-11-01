use postcard::{de_flavors, ser_flavors};
use serde::Deserialize;

use super::*;

struct Pushes(usize);

impl ser_flavors::Flavor for Pushes {
    type Output = usize;

    fn try_push(&mut self, _: u8) -> postcard::Result<()> {
        self.0 += 1;
        Ok(())
    }

    fn try_extend(&mut self, _: &[u8]) -> postcard::Result<()> {
        Ok(())
    }

    fn finalize(self) -> postcard::Result<usize> {
        Ok(self.0)
    }
}

struct Pops<'de> {
    slice: de_flavors::Slice<'de>,
    pops: usize,
}

impl<'de> de_flavors::Flavor<'de> for Pops<'de> {
    type Remainder = usize;
    type Source = &'de [u8];

    fn pop(&mut self) -> postcard::Result<u8> {
        self.pops += 1;
        self.slice.pop()
    }

    fn try_take_n(&mut self, count: usize) -> postcard::Result<&'de [u8]> {
        self.slice.try_take_n(count)
    }

    fn finalize(self) -> postcard::Result<usize> {
        Ok(self.pops)
    }
}

fn pushes<E: wire::Encode<T> + wire::Decode<T>, T>(value: &T) -> usize {
    postcard::serialize_with_flavor(&wire::Encoded::<E, T>::new(value), Pushes(0)).unwrap()
}

fn pops<E: wire::Encode<T> + wire::Decode<T>, T: PartialEq + std::fmt::Debug>(value: &T) -> usize {
    let bytes = wire::encode::<E, T>(value);
    let mut deserializer = postcard::Deserializer::from_flavor(Pops {
        slice: de_flavors::Slice::new(&bytes),
        pops: 0,
    });
    let decoded = wire::Decoded::<E, T>::deserialize(&mut deserializer).unwrap();
    assert_eq!(&decoded.0, value);
    deserializer.finalize().unwrap()
}

fn byte_by_byte<E: wire::Encode<T> + wire::Decode<T>, T: PartialEq + std::fmt::Debug>(
    value: &T,
) -> usize {
    pushes::<E, T>(value).max(pops::<E, T>(value))
}

fn replied<T: PartialEq + std::fmt::Debug, E: wire::Encode<T> + wire::Decode<T>>(
    _: &Reply<T, E>,
    value: &T,
) -> usize {
    byte_by_byte::<E, T>(value)
}

fn awaited<T: PartialEq + std::fmt::Debug, E: wire::Encode<T> + wire::Decode<T>>(
    _: &net::Pending<T, E>,
    value: &T,
) -> usize {
    byte_by_byte::<E, T>(value)
}

fn produced<T: PartialEq + std::fmt::Debug, E: wire::Encode<T> + wire::Decode<T>>(
    _: &Producer<T, E>,
    value: &T,
) -> usize {
    byte_by_byte::<E, T>(value)
}

fn streamed<T: PartialEq + std::fmt::Debug, E: wire::Encode<T> + wire::Decode<T>>(
    _: &net::Stream<T, E>,
    value: &T,
) -> usize {
    byte_by_byte::<E, T>(value)
}

#[test]
fn bytes_cross_whole_in_every_position() {
    let page = vec![7u8; 4096];
    assert!(byte_by_byte::<wire::Plain, Vec<u8>>(&page) >= 4096);

    let pipe = pipe();
    let mut cover = client::shelf::cover(&pipe.client, "Book 1".to_owned());
    let embed::shelf::Call::Cover { reply, .. } = pipe.accept_one() else {
        panic!("the server was not asked for a cover");
    };
    let answer = Ok(page.clone());
    assert!(replied(&reply, &answer) < 8);
    assert!(awaited(&cover, &answer) < 8);
    reply.send(&mut pipe.server.lock(), &answer);
    pipe.pump();
    assert_eq!(poll(&mut cover), std::task::Poll::Ready(Ok(answer)));

    let mut pages = client::shelf::pages(&pipe.client, "Book 1".to_owned());
    let embed::shelf::Call::Pages { stream, .. } = pipe.accept_one() else {
        panic!("the server was not asked for pages");
    };
    assert!(produced(&stream, &page) < 8);
    assert!(streamed(&pages, &page) < 8);
    stream.send(&mut pipe.server.lock(), &page).unwrap();
    pipe.pump();
    assert_eq!(
        poll(&mut pages.recv()),
        std::task::Poll::Ready(Some(page.clone()))
    );

    let scan = shelf::Scan {
        title: "Book 1".to_owned(),
        pages: vec![page.clone(), page.clone()],
    };
    let loose = vec![("cover".to_owned(), page.clone())];
    let args = __args::ShelfBindArgs {
        scan: scan.clone(),
        loose: loose.clone(),
    };
    assert!(pushes::<wire::Plain, _>(&args) < 8);
    let mut bound = client::shelf::bind(&pipe.client, scan.clone(), loose.clone());
    let embed::shelf::Call::Bind {
        reply,
        scan: sent,
        loose: sent_loose,
    } = pipe.accept_one()
    else {
        panic!("the server was not asked to bind");
    };
    assert_eq!((sent, sent_loose), (scan, loose));
    let answer = Some(vec![page.clone(), page]);
    assert!(replied(&reply, &answer) < 8);
    assert!(awaited(&bound, &answer) < 8);
    reply.send(&mut pipe.server.lock(), &answer);
    pipe.pump();
    assert_eq!(poll(&mut bound), std::task::Poll::Ready(Ok(answer)));
}
