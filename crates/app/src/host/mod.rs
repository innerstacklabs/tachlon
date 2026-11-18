//! What the host API keeps on a [`crate::Session`], in its one `host` field: a host row adds a member here and no
//! top-level `Session` field. Today that is the outbox and the key maps. The capabilities
//! (decisions 0016, 0018 and 0019) join it as their rows land.

mod capabilities;
mod gate;
mod keys;
mod outbox;
mod pending;
mod planned;
mod read;
mod results;
mod update;
mod values;

pub(crate) use capabilities::Capabilities;
pub(crate) use keys::Kind;
pub use keys::{KeyTarget, Keys};
pub use outbox::OUTBOX_CAP;
pub(crate) use outbox::Outbox;
pub(crate) use pending::{Pending, PendingKind};
pub(crate) use planned::Plan;
pub use planned::{BeforeEdit, Planned};
pub use update::{UpdateError, Updated};
pub(crate) use values::schedule_update;

/// The host's side of a session. A new session starts with it empty, and `reopen` carries only
/// `read_only`, `before_edit` and `capabilities`, the members that are a choice about the chart and its person.
#[derive(Default)]
pub(crate) struct Host {
    pub(crate) outbox: Outbox,
    /// The commits not yet acknowledged: what a rollback walks.
    pub(crate) pending: Pending,
    pub(crate) keys: Keys,
    pub(crate) read_only: bool,
    pub(crate) capabilities: Capabilities,
    pub(crate) before_edit: Option<BeforeEdit>,
}
