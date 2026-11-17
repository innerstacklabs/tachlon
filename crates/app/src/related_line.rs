//! One line of the hover card's predecessor or successor list: the neighbour's outline path and
//! the link's type and lag as the link editor writes them, `Level 3 / Concrete / Pour slab (SS+2d)`.

use engine::DepId;

use crate::Session;

impl Session {
    /// The line for `neighbour`, reached through link `dep`. `dep` must be a live link.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a dependency index; Schedule::add_dependency casts the count to u32 with no check against u32::MAX, so past 2^32 links it wraps, as that id does"
    )]
    pub(crate) fn related_line(&self, neighbour: u32, dep: usize) -> String {
        let path = self.outline_path(neighbour);
        match self.link_text(DepId(dep as u32)) {
            Some(link) => format!("{path} ({link})"),
            None => path,
        }
    }
}
