use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub fn start(target: SocketAddr, one_way: Duration) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    std::thread::Builder::new()
        .name("latency".into())
        .spawn(move || {
            for client in listener.incoming().flatten() {
                let Ok(server) = TcpStream::connect(target) else {
                    continue;
                };
                let _ = client.set_nodelay(true);
                let _ = server.set_nodelay(true);
                for (from, to) in [(&client, &server), (&server, &client)] {
                    if let (Ok(from), Ok(to)) = (from.try_clone(), to.try_clone()) {
                        delay(from, to, one_way);
                    }
                }
            }
        })?;
    Ok(address)
}

fn delay(mut from: TcpStream, mut to: TcpStream, one_way: Duration) {
    let (sender, receiver) = mpsc::channel::<(Instant, Vec<u8>)>();
    std::thread::spawn(move || {
        let mut buffer = vec![0; 64 * 1024];
        loop {
            match from.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if sender
                        .send((Instant::now() + one_way, buffer[..read].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
    std::thread::spawn(move || {
        for (due, bytes) in receiver {
            let now = Instant::now();
            if due > now {
                std::thread::sleep(due - now);
            }
            if to.write_all(&bytes).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Write);
    });
}
