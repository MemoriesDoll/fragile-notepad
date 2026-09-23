//! A dependency-ordered, deduplicated work list with no application dependencies.

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ScheduleError<Key> {
    MissingDependency(Key),
    Cycle,
}

pub(super) struct UpdateScheduler<Key> {
    operations: Vec<(Key, Vec<Key>)>,
}

impl<Key: Eq + Clone> UpdateScheduler<Key> {
    pub(super) fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Identity is the operation key, independent of registration position.
    /// Re-registering an operation merges its prerequisites and runs it once.
    pub(super) fn register(&mut self, key: Key, after: &[Key]) {
        if let Some((_, dependencies)) = self
            .operations
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            for dependency in after {
                if !dependencies.contains(dependency) {
                    dependencies.push(dependency.clone());
                }
            }
        } else {
            self.operations.push((key, after.to_vec()));
        }
    }

    /// Resolve before executing anything, so invalid dependencies have no partial effects.
    pub(super) fn finish(mut self) -> Result<Vec<Key>, ScheduleError<Key>> {
        for (_, dependencies) in &self.operations {
            for dependency in dependencies {
                if !self.operations.iter().any(|(key, _)| key == dependency) {
                    return Err(ScheduleError::MissingDependency(dependency.clone()));
                }
            }
        }
        let mut ordered = Vec::with_capacity(self.operations.len());
        while !self.operations.is_empty() {
            let Some(index) = self.operations.iter().position(|(_, dependencies)| {
                dependencies
                    .iter()
                    .all(|dependency| ordered.contains(dependency))
            }) else {
                return Err(ScheduleError::Cycle);
            };
            ordered.push(self.operations.remove(index).0);
        }
        Ok(ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependencies_determine_order_and_duplicate_keys_merge_requirements() {
        let mut scheduler = UpdateScheduler::new();
        scheduler.register("persist", &[]);
        scheduler.register("search", &["dispatch"]);
        scheduler.register("persist", &["search"]);
        scheduler.register("dispatch", &[]);
        assert_eq!(
            scheduler.finish(),
            Ok(vec!["dispatch", "search", "persist"])
        );
    }

    #[test]
    fn invalid_graphs_are_rejected_before_execution() {
        let mut missing = UpdateScheduler::new();
        missing.register(1, &[2]);
        assert_eq!(missing.finish(), Err(ScheduleError::MissingDependency(2)));
        let mut cycle = UpdateScheduler::new();
        cycle.register(1, &[2]);
        cycle.register(2, &[1]);
        assert_eq!(cycle.finish(), Err(ScheduleError::Cycle));
        assert_eq!(UpdateScheduler::<u8>::new().finish(), Ok(vec![]));
    }
}
