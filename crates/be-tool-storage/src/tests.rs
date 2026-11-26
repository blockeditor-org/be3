use super::*;

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

mod a_file_goes_whole_with_its_length_and_checksum;
mod nothing_is_sent_without_a_key;
mod put_content_addressed_asks_with_head;
mod put_content_addressed_reads_back_when_head_is_refused;
mod put_if_absent_stores_once_and_refuses_other_bytes;

const KEY: &str = "test key";

/// A request the stub was sent: its method, path and headers, named in lowercase.
struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
}

/// A storage zone on 127.0.0.1 that answers as Bunny's does: `GET` and `PUT` with the key,
/// a `PUT` only with the body's checksum, and `HEAD` without one, as the CDN does. Whether
/// Bunny's storage API takes a `HEAD` with the key is not known, so the stub either answers it
/// as it does one without, or refuses it as 405.
struct Stub {
    url: String,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl Stub {
    fn start() -> Self {
        Self::serving(false)
    }

    fn refusing_keyed_head() -> Self {
        Self::serving(true)
    }

    fn serving(refuses_keyed_head: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let files = Arc::<Mutex<HashMap<String, Vec<u8>>>>::default();
        let requests = Arc::<Mutex<Vec<Request>>>::default();
        let (served, seen) = (files.clone(), requests.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                answer(stream.unwrap(), &served, &seen, refuses_keyed_head);
            }
        });
        Self {
            url,
            files,
            requests,
        }
    }

    fn puts(&self) -> usize {
        self.sent("PUT")
    }

    fn sent(&self, method: &str) -> usize {
        let requests = self.requests.lock().unwrap();
        requests
            .iter()
            .filter(|request| request.method == method)
            .count()
    }
}

fn answer(
    stream: TcpStream,
    files: &Mutex<HashMap<String, Vec<u8>>>,
    requests: &Mutex<Vec<Request>>,
    refuses_keyed_head: bool,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut words = line.split_whitespace();
    let method = words.next().unwrap().to_owned();
    let path = words.next().unwrap().to_owned();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let length = headers
        .get("content-length")
        .map_or(0, |length| length.parse().unwrap());
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let keyed = headers.get("accesskey").map(String::as_str) == Some(KEY);
    let mut files = files.lock().unwrap();
    let (status, sent) = match method.as_str() {
        "HEAD" if keyed && refuses_keyed_head => (405, Vec::new()),
        "HEAD" => (
            if files.contains_key(&path) { 200 } else { 404 },
            Vec::new(),
        ),
        _ if !keyed => (401, Vec::new()),
        "GET" => match files.get(&path) {
            Some(bytes) => (200, bytes.clone()),
            None => (404, Vec::new()),
        },
        "PUT" if headers.get("checksum") == Some(&be_lib_sha256::sha256(&body).to_uppercase()) => {
            files.insert(path.clone(), body);
            (201, Vec::new())
        }
        _ => (400, Vec::new()),
    };
    drop(files);
    requests.lock().unwrap().push(Request {
        method,
        path,
        headers,
    });
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status} Stub\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        sent.len()
    )
    .unwrap();
    stream.write_all(&sent).unwrap();
}
