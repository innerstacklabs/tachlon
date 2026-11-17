//! The outbox (decision 0019, rules 7 and 9): the commits a host has not read yet.

use crate::Session;

/// Most commits the outbox keeps. One past it empties the outbox and sets the snapshot flag.
pub const OUTBOX_CAP: usize = 1000;

#[derive(Default)]
pub(crate) struct Outbox {
    commits: Vec<String>,
    pub(crate) seq: u64,
    /// Commits were dropped since the last read: the host takes a snapshot instead.
    snapshot: bool,
}

impl Outbox {
    pub(crate) fn push(&mut self, commit: String) {
        if self.commits.len() >= OUTBOX_CAP {
            self.commits.clear();
            self.snapshot = true;
        } else if !self.snapshot {
            self.commits.push(commit);
        }
    }
}

impl Session {
    /// The commits made since the last call, oldest first, as JSON (the shape is in `docs/WASM_API.md`);
    /// the outbox is empty after. `snapshot` is true when it passed [`OUTBOX_CAP`] and dropped its commits.
    pub fn take_commits(&mut self) -> String {
        let outbox = &mut self.host.outbox;
        let (commits, seq) = (std::mem::take(&mut outbox.commits), outbox.seq);
        let snapshot = std::mem::take(&mut outbox.snapshot);
        format!(
            r#"{{"commits":[{}],"snapshot":{snapshot},"seq":{seq}}}"#,
            commits.join(",")
        )
    }
}
