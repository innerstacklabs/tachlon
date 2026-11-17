use engine::Instant;

/// Summary-task spans, indexed by task id.
///
/// This is plain data: `app::Session` is its only writer. It is not a copy of
/// `Solution::dates`, and the session rebuilds it on every path that changes
/// the solution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summaries {
    spans: Vec<Option<(Instant, Instant)>>,
}

impl Summaries {
    /// Creates `task_count` unset slots.
    #[must_use]
    pub fn new(task_count: usize) -> Self {
        Self {
            spans: vec![None; task_count],
        }
    }

    /// Returns the number of task slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Returns whether there are no task slots.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Returns a summary's span, or `None` for a leaf, an unknown task, or an
    /// unset slot.
    #[must_use]
    pub fn get(&self, task: u32) -> Option<(Instant, Instant)> {
        self.spans.get(task as usize).copied().flatten()
    }

    /// Sets a task's summary span.
    ///
    /// # Panics
    ///
    /// Panics when `task` is out of range.
    pub fn set(&mut self, task: u32, span: Option<(Instant, Instant)>) {
        self.spans[task as usize] = span;
    }

    /// Returns all task slots.
    #[must_use]
    pub fn as_slice(&self) -> &[Option<(Instant, Instant)>] {
        &self.spans
    }
}

#[cfg(test)]
mod tests {
    use super::Summaries;

    #[test]
    fn get_returns_none_for_out_of_range_task() {
        assert_eq!(Summaries::new(2).get(2), None);
    }

    #[test]
    fn set_get_round_trip() {
        let mut summaries = Summaries::new(2);
        summaries.set(1, Some((10, 20)));
        assert_eq!(summaries.get(1), Some((10, 20)));
        assert_eq!(summaries.as_slice(), &[None, Some((10, 20))]);
    }
}
