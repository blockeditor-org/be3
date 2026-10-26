use std::fmt;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub created: usize,
    pub uploaded: usize,
    pub renamed: usize,
    pub deleted: usize,
    pub written: usize,
    pub moved: usize,
    pub removed: usize,
    pub conflicts: Vec<String>,
    pub skipped: Vec<String>,
    pub unexported: usize,
    pub problems: Vec<String>,
    pub retry: bool,
}

impl Report {
    pub fn absorb(&mut self, pass: Self) {
        self.created += pass.created;
        self.uploaded += pass.uploaded;
        self.renamed += pass.renamed;
        self.deleted += pass.deleted;
        self.written += pass.written;
        self.moved += pass.moved;
        self.removed += pass.removed;
        self.conflicts.extend(pass.conflicts);
        self.skipped = pass.skipped;
        self.unexported = pass.unexported;
        self.problems.extend(pass.problems);
        self.retry = pass.retry;
    }

    pub fn changed(&self) -> bool {
        self.created + self.uploaded + self.renamed + self.deleted > 0
            || self.written + self.moved + self.removed > 0
            || !self.conflicts.is_empty()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = Vec::new();
        let counted = |label: &str, count: usize| (count > 0).then(|| format!("{count} {label}"));
        let server: Vec<String> = [
            counted("created", self.created),
            counted("changed", self.uploaded),
            counted("renamed or moved", self.renamed),
            counted("deleted", self.deleted),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !server.is_empty() {
            lines.push(format!("blocks: {}", server.join(", ")));
        }
        let disk: Vec<String> = [
            counted("written", self.written),
            counted("renamed or moved", self.moved),
            counted("removed", self.removed),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !disk.is_empty() {
            lines.push(format!("files: {}", disk.join(", ")));
        }
        if lines.is_empty() {
            lines.push("already in step".to_owned());
        }
        for conflict in &self.conflicts {
            lines.push(format!("conflict: {conflict}"));
        }
        if !self.skipped.is_empty() {
            lines.push(format!(
                "{} blocks could not be written as files; {} says which and how to fix them",
                self.skipped.len(),
                crate::ERRORS_FILE
            ));
        }
        if self.unexported > 0 {
            lines.push(format!(
                "{} blocks have no plain-file form and stay only in block-app; {}/{} lists them",
                self.unexported,
                crate::STATE_DIRECTORY,
                crate::UNEXPORTED_FILE
            ));
        }
        for problem in &self.problems {
            lines.push(format!("problem: {problem}"));
        }
        if self.retry {
            lines.push("some changes are still to sync; run be sync again".to_owned());
        }
        formatter.write_str(&lines.join("\n"))
    }
}
