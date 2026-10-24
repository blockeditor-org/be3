use std::io::Write;

pub(crate) trait Http {
    fn send(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        into: &mut dyn Write,
    ) -> Result<u16, String>;
}

pub(crate) fn get(
    http: &mut dyn Http,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<(u16, Vec<u8>), String> {
    let mut body = Vec::new();
    let status = http.send("GET", url, headers, &mut body)?;
    Ok((status, body))
}

pub(crate) struct Network;

#[cfg(target_os = "android")]
impl Http for Network {
    fn send(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        into: &mut dyn Write,
    ) -> Result<u16, String> {
        let mut request = crate::android::agent()
            .request(method, url)
            .set("User-Agent", "be-launcher");
        for (name, value) in headers {
            request = request.set(name, value);
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(format!("{url}: {error}.")),
        };
        let status = response.status();
        std::io::copy(&mut response.into_reader(), into)
            .map_err(|error| format!("{url}: {error}."))?;
        Ok(status)
    }
}

#[cfg(not(target_os = "android"))]
impl Http for Network {
    fn send(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        into: &mut dyn Write,
    ) -> Result<u16, String> {
        use std::io::Read as _;
        use std::process::Stdio;

        let mut child = crate::tasks::command("curl")
            .args([
                "--silent",
                "--show-error",
                "--location",
                "--compressed",
                "--user-agent",
                "be-launcher",
                "--request",
                method,
                "--output",
                "-",
                "--write-out",
                "%{stderr}%{http_code}",
                "--config",
                "-",
                url,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not run curl: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let config: String = headers
                .iter()
                .map(|(name, value)| format!("header = \"{name}: {value}\"\n"))
                .collect();
            let _ = stdin.write_all(config.as_bytes());
        }
        let mut stderr = child.stderr.take();
        let errors = std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(stderr) = stderr.as_mut() {
                let _ = stderr.read_to_string(&mut text);
            }
            text
        });
        let mut stdout = child.stdout.take().ok_or("curl gave no output")?;
        let mut buffer = vec![0; 1 << 16];
        let copied = loop {
            match stdout.read(&mut buffer) {
                Ok(0) => break Ok(()),
                Ok(read) => {
                    if let Err(error) = into.write_all(&buffer[..read]) {
                        break Err(error.to_string());
                    }
                }
                Err(error) => break Err(error.to_string()),
            }
        };
        if copied.is_err() {
            let _ = child.kill();
        }
        let status = child
            .wait()
            .map_err(|error| format!("curl failed: {error}"))?;
        let errors = errors.join().unwrap_or_default();
        copied.map_err(|error| format!("{url}: {error}"))?;
        let code = errors.trim().rsplit('\n').next().unwrap_or_default();
        match code.trim().parse::<u16>() {
            Ok(code) if status.success() && code != 0 => Ok(code),
            _ => Err(format!(
                "{url}: {}",
                errors.trim().trim_end_matches(code).trim()
            )),
        }
    }
}
