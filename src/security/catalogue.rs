use serde::{Deserialize, Serialize};

use super::{SecurityCategory, SecurityError};

const MATCHERS_TOML: &str = include_str!("security_matchers.toml");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct MatcherCatalogue {
    pub(super) version: usize,
    pub(super) matchers: Vec<SecurityMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct SecurityMatcher {
    pub(super) category: SecurityCategory,
    pub(super) rule_id: String,
    pub(super) detector: String,
    pub(super) sink_shape: String,
    pub(super) argument_index: usize,
    pub(super) literal_predicate: String,
    pub(super) context_predicate: String,
    pub(super) import_provenance: String,
    pub(super) callees: Vec<String>,
    pub(super) cwe: Vec<String>,
    pub(super) effect: String,
    pub(super) evidence_template: String,
    pub(super) source: String,
    pub(super) boundary: String,
    pub(super) trace_role: String,
    pub(super) default_severity: SecurityDefaultSeverity,
    pub(super) verification_prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityDefaultSeverity {
    Warning,
    Error,
}

impl MatcherCatalogue {
    pub(super) fn parse() -> Result<Self, SecurityError> {
        let catalogue =
            toml::from_str::<Self>(MATCHERS_TOML).map_err(SecurityError::MatcherCatalogue)?;
        catalogue.validate()?;
        Ok(catalogue)
    }

    pub(super) fn matcher(&self, category: SecurityCategory) -> &SecurityMatcher {
        self.matchers
            .iter()
            .find(|matcher| matcher.category == category)
            .unwrap_or_else(|| unreachable!("validated matcher catalogue covers every category"))
    }

    pub(super) fn by_detector(&self, detector: &str) -> &SecurityMatcher {
        self.matchers
            .iter()
            .find(|matcher| matcher.detector == detector)
            .unwrap_or_else(|| unreachable!("validated matcher catalogue covers every detector"))
    }

    fn validate(&self) -> Result<(), SecurityError> {
        if self.version != 1 {
            return Err(SecurityError::InvalidMatcherCatalogue(format!(
                "unsupported version {}",
                self.version
            )));
        }
        for category in SecurityCategory::ALL {
            let matches = self
                .matchers
                .iter()
                .filter(|matcher| matcher.category == category)
                .count();
            if matches != 1 {
                return Err(SecurityError::InvalidMatcherCatalogue(format!(
                    "expected one matcher for {category:?}, found {matches}"
                )));
            }
        }
        for matcher in &self.matchers {
            if matcher.rule_id.is_empty()
                || matcher.cwe.is_empty()
                || matcher.effect.is_empty()
                || matcher.evidence_template.is_empty()
                || matcher.source.is_empty()
                || matcher.boundary.is_empty()
                || matcher.trace_role.is_empty()
                || matcher.sink_shape.is_empty()
                || matcher.literal_predicate.is_empty()
                || matcher.context_predicate.is_empty()
                || matcher.import_provenance.is_empty()
            {
                return Err(SecurityError::InvalidMatcherCatalogue(format!(
                    "matcher {} has an empty contract field",
                    matcher.detector
                )));
            }
        }
        Ok(())
    }
}
