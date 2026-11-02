use std::io::Write;

const DROPPED: [&str; 9] = [
    "Build ID:",
    "File changed:",
    "Directory changed:",
    "RE Session:",
    "Cache hits:",
    "Commands:",
    "Network:",
    "Resource usage:",
    "IO:",
];

pub struct Quiet {
    live: bool,
    width: usize,
    passed: Option<String>,
    skipping: bool,
    showing: bool,
}

impl Quiet {
    pub fn new(live: bool, width: usize) -> Quiet {
        Quiet {
            live,
            width,
            passed: None,
            skipping: false,
            showing: false,
        }
    }

    pub fn line(&mut self, line: &str, out: &mut dyn Write) {
        let stamp = stamp_of(line);
        let mut text = &line[stamp.len()..];
        if !stamp.is_empty() {
            text = text.strip_prefix(' ').unwrap_or(text);
        }

        if let Some(passed) = &self.passed
            && !passed.is_empty()
            && passed == stamp
            && !text.starts_with("✓ ")
            && !text.starts_with("✗ ")
        {
            return;
        }
        self.passed = None;

        if text.starts_with("✓ Pass: ") {
            self.passed = Some(stamp.to_owned());
            return;
        }
        if text.starts_with("Skipped ") && is_skipped_header(text) {
            self.skipping = true;
            return;
        }
        if self.skipping && text.starts_with("  ") {
            return;
        }
        self.skipping = false;

        if DROPPED.iter().any(|prefix| text.starts_with(prefix)) {
            return;
        }
        if text.starts_with("Waiting on ") {
            self.status(text, out);
            return;
        }
        if is_change_events(text)
            || text.starts_with("BUILD SUCCEEDED")
            || text.starts_with("BXL SUCCEEDED")
            || (text.starts_with("test ") && text.ends_with(" ... ok"))
        {
            return;
        }
        self.say(text, out);
    }

    pub fn finish(&mut self, out: &mut dyn Write) {
        if self.showing {
            let _ = write!(out, "\r\x1b[K");
            let _ = out.flush();
            self.showing = false;
        }
    }

    fn status(&mut self, text: &str, out: &mut dyn Write) {
        if !self.live {
            return;
        }
        let shown: String = text.chars().take(self.width.saturating_sub(1)).collect();
        let _ = write!(out, "\r\x1b[K{shown}");
        let _ = out.flush();
        self.showing = true;
    }

    fn say(&mut self, text: &str, out: &mut dyn Write) {
        if self.showing {
            let _ = write!(out, "\r\x1b[K");
            self.showing = false;
        }
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

fn stamp_of(line: &str) -> &str {
    if line.starts_with('[')
        && let Some(end) = line.find(']')
    {
        return &line[..=end];
    }
    ""
}

fn is_skipped_header(text: &str) -> bool {
    text.strip_prefix("Skipped ")
        .and_then(|rest| {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            (digits > 0).then(|| &rest[digits..])
        })
        .is_some_and(|rest| rest.starts_with(" incompatible targets:"))
}

fn is_change_events(text: &str) -> bool {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && text[digits..].starts_with(" additional file change events")
}
