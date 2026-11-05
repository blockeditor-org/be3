use super::*;

#[test]
fn a_presigned_put_takes_only_the_object_it_was_signed_for() {
    let server = TestBucket::start();
    let bucket = server.bucket();
    let store = S3Store::new(bucket.clone());
    let object = b"an object a client uploads itself";
    let hash = Hash::of(object);
    let presigned = bucket.presign_put(hash, object.len() as u64, Duration::from_secs(60));
    assert_eq!(
        presigned.headers,
        vec![
            ("content-length", object.len().to_string()),
            ("x-amz-checksum-sha256", checksum(hash)),
        ]
    );
    let put = |body: &[u8], headers: &[(&str, String)]| {
        let mut request = agent().put(&presigned.url);
        for (name, value) in headers {
            request = request.set(name, value);
        }
        match request.send_bytes(body) {
            Ok(response) => response.status(),
            Err(ureq::Error::Status(status, _)) => status,
            Err(error) => panic!("{error}"),
        }
    };

    let same_length = b"an object a client forges instead";
    assert_eq!(same_length.len(), object.len());
    assert_eq!(put(same_length, &presigned.headers), 400);
    assert_eq!(store.get(hash).unwrap(), None);

    let longer = b"an object a client uploads itself, and more";
    let mut lying = presigned.headers.clone();
    lying[0].1 = longer.len().to_string();
    assert_eq!(put(longer, &lying), 403);
    assert_eq!(put(object, &presigned.headers[..1]), 403);
    assert_eq!(store.get(hash).unwrap(), None);

    assert_eq!(put(object, &presigned.headers), 200);
    assert_eq!(store.get(hash).unwrap().as_deref(), Some(&object[..]));
}
