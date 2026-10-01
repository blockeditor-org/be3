use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener as StdListener,
    sync::{Arc, Mutex},
};

use crate::backup::{Bunny, Target};

fn serve_zone(listener: StdListener, files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { return };
        let files = Arc::clone(&files);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let mut parts = line.split_whitespace();
                let (method, path) = (
                    parts.next().unwrap().to_owned(),
                    parts.next().unwrap().to_owned(),
                );
                let (mut length, mut key) = (0, String::new());
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap();
                    match name.to_ascii_lowercase().as_str() {
                        "content-length" => length = value.trim().parse().unwrap(),
                        "accesskey" => key = value.trim().to_owned(),
                        _ => {}
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let path = path.strip_prefix("/zone/").unwrap_or("").to_owned();
                let mut files = files.lock().unwrap();
                let (status, reply) = if key != "zone password" {
                    (401, Vec::new())
                } else if method == "PUT" {
                    files.insert(path, body);
                    (201, Vec::new())
                } else if method == "DELETE" {
                    files.remove(&path);
                    (200, Vec::new())
                } else if let Some(directory) = path.strip_suffix('/') {
                    let listed: Vec<String> = files
                        .keys()
                        .filter_map(|name| name.strip_prefix(&format!("{directory}/")))
                        .filter(|name| !name.contains('/'))
                        .map(|name| format!(r#"{{"ObjectName":"{name}","IsDirectory":false}}"#))
                        .collect();
                    (200, format!("[{}]", listed.join(",")).into_bytes())
                } else {
                    match files.get(&path) {
                        Some(bytes) => (200, bytes.clone()),
                        None => (404, Vec::new()),
                    }
                };
                drop(files);
                write!(
                    writer,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\n\r\n",
                    reply.len()
                )
                .unwrap();
                writer.write_all(&reply).unwrap();
            }
        });
    }
}

#[test]
fn a_bunny_storage_zone_holds_a_backup() {
    let listener = StdListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let files = Arc::new(Mutex::new(BTreeMap::new()));
    let held = Arc::clone(&files);
    std::thread::spawn(move || serve_zone(listener, held));

    let zone = Bunny::new(&endpoint, "zone", "zone password".to_owned());
    zone.put("databases/one.sealed", b"first").unwrap();
    zone.put("databases/two.sealed", b"second").unwrap();
    zone.put("objects/ab/cdef", b"object").unwrap();
    assert_eq!(
        zone.get("databases/two.sealed").unwrap(),
        Some(b"second".to_vec())
    );
    assert_eq!(zone.get("databases/three.sealed").unwrap(), None);
    let mut listed = zone.list("databases").unwrap();
    listed.sort();
    assert_eq!(listed, vec!["one.sealed", "two.sealed"]);
    zone.delete("databases/one.sealed").unwrap();
    assert_eq!(zone.list("databases").unwrap(), vec!["two.sealed"]);
    assert!(files.lock().unwrap().contains_key("objects/ab/cdef"));

    let intruder = Bunny::new(&endpoint, "zone", "a guess".to_owned());
    assert!(intruder.put("databases/forged.sealed", b"forged").is_err());
}
