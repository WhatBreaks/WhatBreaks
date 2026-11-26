//! Select and assess compatibility claims for a proposed version transition.

use std::cmp::Ordering;

use crate::compat_evidence::{
    derive_finding_status, ClaimEffect, FindingStatus, KnowledgeEvidence,
};
use crate::version::{VersionOrdering, VersionTransition};

/// Why a transition has no assessed claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityAssessmentReason {
    /// At least one version is explicitly unknown.
    UnknownVersion,
    /// The versions use different schemes or cannot be ordered.
    IncomparableVersions,
    /// The target is the same version or an older version.
    NotAnUpgrade,
    /// The upgrade has no applicable official evidence.
    NoApplicableEvidence,
}

/// Compatibility claims selected for one proposed version transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionCompatibilityAssessment<'a> {
    /// Strength of the selected claims. Verified describes evidence, not upgrade safety.
    pub status: FindingStatus,
    /// Present when no claims could be assessed.
    pub reason: Option<CompatibilityAssessmentReason>,
    /// Known breaking claims, with sources, ordered by evidence ID.
    pub breaking_claims: Vec<&'a KnowledgeEvidence>,
    /// Known non-breaking claims, with sources, ordered by evidence ID.
    pub non_breaking_claims: Vec<&'a KnowledgeEvidence>,
}

impl VersionCompatibilityAssessment<'_> {
    fn unknown(reason: CompatibilityAssessmentReason) -> Self {
        Self {
            status: FindingStatus::Unknown,
            reason: Some(reason),
            breaking_claims: Vec::new(),
            non_breaking_claims: Vec::new(),
        }
    }
}

/// Analyze an upgrade against supplied records without fetching external data.
///
/// Evidence applies when its ranges cover the current and target versions,
/// including claims introduced in an intermediate release. A non-breaking
/// claim applies to the behavior described by that source. It cannot establish
/// that the entire upgrade is safe.
pub fn analyze_version_compatibility<'a>(
    transition: &VersionTransition,
    records: &'a [KnowledgeEvidence],
    ordering: &dyn VersionOrdering,
) -> VersionCompatibilityAssessment<'a> {
    use CompatibilityAssessmentReason::{
        IncomparableVersions, NoApplicableEvidence, NotAnUpgrade, UnknownVersion,
    };

    let current = &transition.current;
    let target = &transition.target;
    let reason = if current.is_unknown() || target.is_unknown() {
        Some(UnknownVersion)
    } else if current.scheme != target.scheme {
        Some(IncomparableVersions)
    } else {
        match ordering.compare(&current.scheme, &current.raw, &target.raw) {
            Some(Ordering::Less) => None,
            Some(Ordering::Equal | Ordering::Greater) => Some(NotAnUpgrade),
            None => Some(IncomparableVersions),
        }
    };
    if let Some(reason) = reason {
        return VersionCompatibilityAssessment::unknown(reason);
    }

    let mut applicable: Vec<_> = records
        .iter()
        .filter(|record| record.is_applicable_to(transition, ordering))
        .collect();
    if applicable.is_empty() {
        return VersionCompatibilityAssessment::unknown(NoApplicableEvidence);
    }
    applicable.sort_by(|left, right| left.id.cmp(&right.id));
    let status = derive_finding_status(transition, &applicable, false, ordering);
    let (breaking_claims, non_breaking_claims) = applicable
        .into_iter()
        .partition(|record| record.effect == ClaimEffect::Breaking);

    VersionCompatibilityAssessment {
        status,
        reason: None,
        breaking_claims,
        non_breaking_claims,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat_evidence::{KnowledgeEvidenceId, SourceAuthority, SourceRef, VersionRange};
    use crate::version::{DottedNumericOrdering, VersionRef};

    fn claim(id: &str, effect: ClaimEffect) -> KnowledgeEvidence {
        KnowledgeEvidence {
            id: KnowledgeEvidenceId::new(id),
            component: "postgresql".into(),
            applies_from: VersionRange::new("semver", None, Some("16.0".into()), true, false),
            applies_to: VersionRange::new("semver", Some("16.0".into()), None, true, false),
            effect,
            claim: format!("Compatibility claim {id}"),
            source: SourceRef {
                url: format!("https://example.test/{id}"),
                title: format!("Release notes {id}"),
                reviewed_on: "2026-01-01".into(),
                content_hash: format!("hash-{id}"),
                authority: SourceAuthority::Official,
            },
        }
    }

    fn upgrade(from: &str, to: &str) -> VersionTransition {
        VersionTransition::new(
            "postgresql",
            VersionRef::known("semver", from),
            VersionRef::known("semver", to),
        )
    }

    #[test]
    fn selects_intermediate_release_claims_and_groups_them_by_effect() {
        let unrelated = KnowledgeEvidence {
            component: "other".into(),
            ..claim("unrelated", ClaimEffect::Breaking)
        };
        let claims = [
            claim("c", ClaimEffect::Breaking),
            claim("a", ClaimEffect::Breaking),
            claim("b", ClaimEffect::NonBreaking),
            unrelated,
        ];
        let result = analyze_version_compatibility(
            &upgrade("15.4", "17.0"),
            &claims,
            &DottedNumericOrdering,
        );

        assert_eq!(result.status, FindingStatus::Conflicting);
        assert_eq!(result.reason, None);
        assert_eq!(result.breaking_claims, [&claims[1], &claims[0]]);
        assert_eq!(result.non_breaking_claims, [&claims[2]]);
        assert_eq!(
            result.breaking_claims[0].source.url,
            "https://example.test/a"
        );
    }

    #[test]
    fn distinguishes_missing_evidence_from_unsupported_transitions() {
        use CompatibilityAssessmentReason::*;
        let claims = [claim("a", ClaimEffect::Breaking)];
        let mut unknown = upgrade("15.4", "17.0");
        unknown.current = VersionRef::unknown("semver");
        let mut incomparable = upgrade("15.4", "17.0");
        incomparable.target = VersionRef::known("other", "17.0");
        let invalid = upgrade("15.4", "invalid");
        for (transition, reason) in [
            (upgrade("15.4", "15.9"), NoApplicableEvidence),
            (upgrade("16.0", "16.0.0"), NotAnUpgrade),
            (upgrade("17.0", "15.4"), NotAnUpgrade),
            (unknown, UnknownVersion),
            (incomparable, IncomparableVersions),
            (invalid, IncomparableVersions),
        ] {
            let result =
                analyze_version_compatibility(&transition, &claims, &DottedNumericOrdering);
            assert_eq!(result.status, FindingStatus::Unknown);
            assert_eq!(result.reason, Some(reason));
            assert!(result.breaking_claims.is_empty());
            assert!(result.non_breaking_claims.is_empty());
        }
    }

    #[test]
    fn community_only_claims_are_not_applicable_evidence() {
        let mut community = claim("community", ClaimEffect::Breaking);
        community.source.authority = SourceAuthority::Community;
        let claims = [community];
        let result = analyze_version_compatibility(
            &upgrade("15.4", "17.0"),
            &claims,
            &DottedNumericOrdering,
        );
        assert_eq!(result.status, FindingStatus::Unknown);
        assert_eq!(
            result.reason,
            Some(CompatibilityAssessmentReason::NoApplicableEvidence)
        );
        assert!(result.breaking_claims.is_empty());
        assert!(result.non_breaking_claims.is_empty());
    }

    #[test]
    fn non_breaking_only_returns_the_claim_and_its_source() {
        let claims = [claim("a", ClaimEffect::NonBreaking)];
        let result = analyze_version_compatibility(
            &upgrade("15.4", "16.0"),
            &claims,
            &DottedNumericOrdering,
        );
        assert_eq!(result.status, FindingStatus::Verified);
        assert_eq!(result.reason, None);
        assert!(result.breaking_claims.is_empty());
        assert_eq!(result.non_breaking_claims, [&claims[0]]);
    }

    #[test]
    fn inferred_version_caps_evidence_at_inferred() {
        let mut transition = upgrade("15.4", "16.0");
        transition.target = VersionRef::inferred("semver", "16.0");
        let claims = [claim("a", ClaimEffect::Breaking)];
        let result = analyze_version_compatibility(&transition, &claims, &DottedNumericOrdering);
        assert_eq!(result.status, FindingStatus::Inferred);
        assert_eq!(result.breaking_claims, [&claims[0]]);
    }
}
