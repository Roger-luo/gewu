use std::collections::BTreeMap;

use gewu_review::load::Criterion;

use crate::{DecideError, Decider};

/// Scripted answers, for tests and offline runs: `p` for every criterion
/// unless `script` names it.
#[derive(Clone, Debug, Default)]
pub struct MockDecider {
    pub p: f64,
    pub script: BTreeMap<String, f64>,
    /// Fail every call, to exercise the fallback to reading time.
    pub fail: bool,
}

impl Decider for MockDecider {
    fn answer(
        &self,
        _state: &str,
        criteria: &[Criterion],
    ) -> Result<BTreeMap<String, f64>, DecideError> {
        if self.fail {
            return Err(DecideError::Network("scripted failure".into()));
        }
        Ok(criteria
            .iter()
            .map(|criterion| {
                let p = self.script.get(&criterion.name).copied().unwrap_or(self.p);
                (criterion.name.clone(), p)
            })
            .collect())
    }
}
