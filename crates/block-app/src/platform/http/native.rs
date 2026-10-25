use std::{io::Read, thread, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn fetch(
    url: String,
    headers: Vec<(&'static str, String)>,
    aborted: super::Aborted,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    thread::spawn(move || {
        let fetched = run(&url, &headers, &aborted);
        if !aborted.aborted() {
            deliver(fetched);
        }
    });
}

fn run(
    url: &str,
    headers: &[(&'static str, String)],
    aborted: &super::Aborted,
) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).build();
    let mut request = agent.get(url);
    for (key, value) in headers {
        request = request.set(key, value);
    }
    let response = match request.call() {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.to_string()),
    };
    if aborted.aborted() {
        return Err(format!("{url} was no longer wanted"));
    }
    let status = response.status();
    let limit = super::MAX_BODY_BYTES;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    if body.len() > limit {
        return Err(format!("{url} sent more than {limit} bytes"));
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}: {}", String::from_utf8_lossy(&body)));
    }
    Ok(body)
}
