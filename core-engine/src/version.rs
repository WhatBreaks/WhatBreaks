//! WhatBreaks Version Transition Modeling
//!
//! Authoritative domain models for representing component versions, version
//! knowledge/uncertainty states, version comparison schemes, and proposed
//! version transitions.
//!
//! # Design Principles
//!
//! - **Provider-Agnostic**: Components, version strings, and schemes are open
//!   domain types. This crate does not import Kubernetes, AWS, or database SDKs.
//! - **Deterministic & Pure**: Zero system clock, zero randomness, and zero I/O.
//! - **Explicit Uncertainty**: Unknown versions are represented explicitly via
//!   [`VersionKnowledge::Unknown`] rather than missing fields or empty strings.
//! - **Scheme-Aware**: Extensible comparison via [`VersionOrdering`] trait.
//! - **Standard Dotted-Numeric Comparison**: Robust numeric ordering supporting
//!   multi-component numbers, zero-padding normalization, and numeric vs. lexical
//!   sorting (e.g. `1.10` > `1.9`).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// 1. VersionKnowledge
// ---------------------------------------------------------------------------

/// Epistemological certainty of a version string.
///
/// An unknown version is represented explicitly with [`VersionKnowledge::Unknown`]
/// rather than by an absent or empty string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VersionKnowledge {
    /// The version was directly stated, observed, or configured.
    Known,
    /// The version was derived indirectly (for example parsed from an image tag or label).
    Inferred,
    /// The version is not known at all.
    Unknown,
}

impl VersionKnowledge {
    /// Return the canonical uppercase string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Known => "KNOWN",
            Self::Inferred => "INFERRED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Return `true` only for [`VersionKnowledge::Known`].
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known)
    }

    /// Return `true` only for [`VersionKnowledge::Inferred`].
    pub fn is_inferred(&self) -> bool {
        matches!(self, Self::Inferred)
    }

    /// Return `true` only for [`VersionKnowledge::Unknown`].
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

// ---------------------------------------------------------------------------
// 2. VersionRef
// ---------------------------------------------------------------------------

/// A component version reference together with its certainty and interpretation scheme.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionRef {
    /// The versioning scheme used to interpret `raw` (e.g. "dotted", "semver", "k8s").
    pub scheme: String,
    /// The exact version string as provided (empty string when Unknown).
    pub raw: String,
    /// Epistemological certainty of this version reference.
    pub knowledge: VersionKnowledge,
}

impl VersionRef {
    /// Construct a new version reference with explicit knowledge.
    pub fn new(
        scheme: impl Into<String>,
        raw: impl Into<String>,
        knowledge: VersionKnowledge,
    ) -> Self {
        Self {
            scheme: scheme.into(),
            raw: raw.into(),
            knowledge,
        }
    }

    /// Construct a directly known version reference.
    pub fn known(scheme: impl Into<String>, raw: impl Into<String>) -> Self {
        Self::new(scheme, raw, VersionKnowledge::Known)
    }

    /// Construct an indirectly inferred version reference.
    pub fn inferred(scheme: impl Into<String>, raw: impl Into<String>) -> Self {
        Self::new(scheme, raw, VersionKnowledge::Inferred)
    }

    /// Construct an explicitly unknown version reference.
    pub fn unknown(scheme: impl Into<String>) -> Self {
        Self::new(scheme, "", VersionKnowledge::Unknown)
    }

    /// Check if the version is known.
    pub fn is_known(&self) -> bool {
        self.knowledge.is_known()
    }

    /// Check if the version is inferred.
    pub fn is_inferred(&self) -> bool {
        self.knowledge.is_inferred()
    }

    /// Check if the version is unknown.
    pub fn is_unknown(&self) -> bool {
        self.knowledge.is_unknown()
    }
}

// ---------------------------------------------------------------------------
// 3. VersionOrdering & DottedNumericOrdering
// ---------------------------------------------------------------------------

/// Scheme-aware comparison of two version strings.
///
/// Returns `None` if either version string cannot be parsed under the given scheme
/// or if the scheme is unrecognized.
pub trait VersionOrdering: Send + Sync {
    /// Compare version `a` with version `b` under `scheme`.
    fn compare(&self, scheme: &str, a: &str, b: &str) -> Option<Ordering>;
}

/// Compares dot-separated numeric versions (e.g. "15.4" vs "16.1", "1.9" vs "1.10").
///
/// # Rules
/// - Accepts non-negative integers separated by single dots.
/// - Strips an optional leading `'v'` or `'V'` (e.g. `"v1.28.0"` -> `"1.28.0"`).
/// - Segments are compared numerically: 10 > 9 (unlike lexical sorting where `"1.10"` < `"1.9"`).
/// - Shorter version numbers are zero-padded on the right (`1.2` == `1.2.0`).
/// - Returns `None` for empty strings, non-numeric parts, signs, or invalid punctuation.
/// - Scheme validation: recognizes dot-separated numeric schemes (`"dotted"`, `"dotted-numeric"`,
///   and numeric component releases under `"semver"`). Returns `None` for unrecognized schemes
///   or non-numeric extensions (such as semver prerelease tags).
#[derive(Debug, Clone, Copy, Default)]
pub struct DottedNumericOrdering;

impl DottedNumericOrdering {
    /// Return whether this comparator supports the given versioning scheme.
    ///
    /// Currently recognizes dot-separated numeric schemes: `"dotted"`, `"dotted-numeric"`,
    /// and numeric releases under `"semver"` (preserving downstream `compat_evidence` compatibility).
    #[inline]
    pub fn supports_scheme(scheme: &str) -> bool {
        matches!(scheme, "dotted" | "dotted-numeric" | "semver")
    }
}

impl VersionOrdering for DottedNumericOrdering {
    /// Compare two version strings under the specified scheme.
    ///
    /// # Arguments
    /// * `scheme` - The version scheme (e.g. `"dotted"`, `"dotted-numeric"`, or `"semver"`).
    /// * `a` - The first version string.
    /// * `b` - The second version string.
    ///
    /// # Returns
    /// * `Some(Ordering::Equal)` if the numeric components are identical under zero-padding.
    /// * `Some(Ordering::Greater)` if `a` is greater than `b`.
    /// * `Some(Ordering::Less)` if `a` is less than `b`.
    /// * `None` if `scheme` is unsupported or either string contains invalid or non-numeric tokens.
    fn compare(&self, scheme: &str, a: &str, b: &str) -> Option<Ordering> {
        if !Self::supports_scheme(scheme) {
            return None;
        }
        let left = parse_dotted(a)?;
        let right = parse_dotted(b)?;
        let max_len = left.len().max(right.len());
        for i in 0..max_len {
            let l = left.get(i).copied().unwrap_or(0);
            let r = right.get(i).copied().unwrap_or(0);
            match l.cmp(&r) {
                Ordering::Equal => continue,
                non_eq => return Some(non_eq),
            }
        }
        Some(Ordering::Equal)
    }
}

/// Parse a dot-separated numeric version string into a vector of integer components.
///
/// Strips an optional leading `'v'` or `'V'`, and parses each dot-separated segment
/// as a `u64`. Returns `None` if the input is empty, has empty segments (e.g. `"1..2"`),
/// contains non-digit characters, or has numbers that overflow `u64`.
fn parse_dotted(s: &str) -> Option<Vec<u64>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let clean = s.strip_prefix(|c| c == 'v' || c == 'V').unwrap_or(s);
    if clean.is_empty() {
        return None;
    }
    clean
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                None
            } else {
                part.parse::<u64>().ok()
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 4. VersionTransition
// ---------------------------------------------------------------------------

/// A proposed move of one component from a current version to a target version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionTransition {
    /// The software component undergoing transition (e.g. "postgresql", "kube-apiserver").
    pub component: String,
    /// The current running version.
    pub current: VersionRef,
    /// The proposed target version.
    pub target: VersionRef,
}

impl VersionTransition {
    /// Create a new version transition.
    pub fn new(component: impl Into<String>, current: VersionRef, target: VersionRef) -> Self {
        Self {
            component: component.into(),
            current,
            target,
        }
    }

    /// Check if this transition represents a verified upgrade under the provided ordering.
    ///
    /// Returns `true` only if:
    /// 1. Both `current` and `target` use the same versioning scheme.
    /// 2. Neither `current` nor `target` is Unknown.
    /// 3. Ordering strictly evaluates to `Ordering::Less` (target is newer than current).
    pub fn is_upgrade(&self, ord: &dyn VersionOrdering) -> bool {
        if self.current.is_unknown() || self.target.is_unknown() {
            return false;
        }
        if self.current.scheme != self.target.scheme {
            return false;
        }
        ord.compare(&self.current.scheme, &self.current.raw, &self.target.raw)
            == Some(Ordering::Less)
    }

    /// Check if this transition represents a verified downgrade under the provided ordering.
    ///
    /// Returns `true` only if:
    /// 1. Both `current` and `target` use the same versioning scheme.
    /// 2. Neither `current` nor `target` is Unknown.
    /// 3. Ordering strictly evaluates to `Ordering::Greater` (current is newer than target).
    pub fn is_downgrade(&self, ord: &dyn VersionOrdering) -> bool {
        if self.current.is_unknown() || self.target.is_unknown() {
            return false;
        }
        if self.current.scheme != self.target.scheme {
            return false;
        }
        ord.compare(&self.current.scheme, &self.current.raw, &self.target.raw)
            == Some(Ordering::Greater)
    }

    /// Check if this transition is a no-op (equal versions under the scheme).
    ///
    /// Returns `true` only if:
    /// 1. Both `current` and `target` use the same versioning scheme.
    /// 2. Neither `current` nor `target` is Unknown.
    /// 3. Ordering strictly evaluates to `Ordering::Equal` (target is identical to current under zero-padding).
    pub fn is_noop(&self, ord: &dyn VersionOrdering) -> bool {
        if self.current.is_unknown() || self.target.is_unknown() {
            return false;
        }
        if self.current.scheme != self.target.scheme {
            return false;
        }
        ord.compare(&self.current.scheme, &self.current.raw, &self.target.raw)
            == Some(Ordering::Equal)
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ord() -> DottedNumericOrdering {
        DottedNumericOrdering
    }

    // --- VersionKnowledge Tests ---

    #[test]
    fn test_version_knowledge_predicates_and_string() {
        assert!(VersionKnowledge::Known.is_known());
        assert!(!VersionKnowledge::Known.is_inferred());
        assert!(!VersionKnowledge::Known.is_unknown());
        assert_eq!(VersionKnowledge::Known.as_str(), "KNOWN");

        assert!(!VersionKnowledge::Inferred.is_known());
        assert!(VersionKnowledge::Inferred.is_inferred());
        assert!(!VersionKnowledge::Inferred.is_unknown());
        assert_eq!(VersionKnowledge::Inferred.as_str(), "INFERRED");

        assert!(!VersionKnowledge::Unknown.is_known());
        assert!(!VersionKnowledge::Unknown.is_inferred());
        assert!(VersionKnowledge::Unknown.is_unknown());
        assert_eq!(VersionKnowledge::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn test_version_knowledge_serde_roundtrip() {
        for (variant, expected_json) in [
            (VersionKnowledge::Known, "\"KNOWN\""),
            (VersionKnowledge::Inferred, "\"INFERRED\""),
            (VersionKnowledge::Unknown, "\"UNKNOWN\""),
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            assert_eq!(json, expected_json);
            let parsed: VersionKnowledge = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, variant);
        }
    }

    // --- VersionRef Tests ---

    #[test]
    fn test_version_ref_constructors() {
        let known = VersionRef::known("dotted", "15.4");
        assert_eq!(known.scheme, "dotted");
        assert_eq!(known.raw, "15.4");
        assert!(known.is_known());
        assert!(!known.is_inferred());
        assert!(!known.is_unknown());

        let inferred = VersionRef::inferred("semver", "1.2.3");
        assert_eq!(inferred.scheme, "semver");
        assert_eq!(inferred.raw, "1.2.3");
        assert!(!inferred.is_known());
        assert!(inferred.is_inferred());
        assert!(!inferred.is_unknown());

        let unknown = VersionRef::unknown("dotted");
        assert_eq!(unknown.scheme, "dotted");
        assert_eq!(unknown.raw, "");
        assert!(!unknown.is_known());
        assert!(!unknown.is_inferred());
        assert!(unknown.is_unknown());
    }

    #[test]
    fn test_version_ref_serde_roundtrip() {
        let v = VersionRef::known("dotted", "16.1");
        let json = serde_json::to_string(&v).expect("serialize");
        let parsed: VersionRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, v);
    }

    // --- DottedNumericOrdering Tests ---

    #[test]
    fn test_dotted_numeric_numerical_ordering_1_10_vs_1_9() {
        // Critical requirement: 1.10 must be strictly greater than 1.9
        let cmp = ord().compare("dotted", "1.10", "1.9");
        assert_eq!(cmp, Some(Ordering::Greater));

        let cmp_rev = ord().compare("dotted", "1.9", "1.10");
        assert_eq!(cmp_rev, Some(Ordering::Less));
    }

    #[test]
    fn test_dotted_numeric_zero_padding_normalization() {
        // Critical requirement: 1.2 must equal 1.2.0 and 1.2.0.0
        assert_eq!(
            ord().compare("dotted", "1.2", "1.2.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "1.2.0", "1.2"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "1.2", "1.2.0.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "1.2.0.0", "1.2.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(ord().compare("dotted", "0", "0.0.0"), Some(Ordering::Equal));
    }

    #[test]
    fn test_dotted_numeric_multi_component_comparison() {
        assert_eq!(
            ord().compare("dotted", "15.4.1", "15.4.2"),
            Some(Ordering::Less)
        );
        assert_eq!(
            ord().compare("dotted", "15.4.2", "15.4.1"),
            Some(Ordering::Greater)
        );
        assert_eq!(
            ord().compare("dotted", "15.4.1", "15.4.1"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "15.4", "16.0"),
            Some(Ordering::Less)
        );
        assert_eq!(
            ord().compare("dotted", "17.0", "15.4"),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn test_dotted_numeric_v_prefix_support() {
        assert_eq!(
            ord().compare("dotted", "v1.28.0", "1.28.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "V1.28.0", "1.28.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("dotted", "v1.28.0", "v1.29.0"),
            Some(Ordering::Less)
        );
    }

    #[test]
    fn test_dotted_numeric_invalid_and_empty_inputs() {
        assert_eq!(ord().compare("dotted", "", "1.0"), None);
        assert_eq!(ord().compare("dotted", "1.0", ""), None);
        assert_eq!(ord().compare("dotted", "abc", "1.0"), None);
        assert_eq!(ord().compare("dotted", "1..2", "1.2"), None);
        assert_eq!(ord().compare("dotted", "1.2-alpha", "1.2"), None);
        assert_eq!(ord().compare("dotted", "-1.0", "1.0"), None);
        assert_eq!(ord().compare("dotted", "1.0", "1.0.invalid"), None);
        assert_eq!(ord().compare("dotted", "v", "1.0"), None);
    }

    #[test]
    fn test_dotted_numeric_scheme_support_and_rejection() {
        // Supported schemes
        assert_eq!(ord().compare("dotted", "1.0", "2.0"), Some(Ordering::Less));
        assert_eq!(
            ord().compare("dotted-numeric", "1.0", "2.0"),
            Some(Ordering::Less)
        );
        assert_eq!(
            ord().compare("semver", "1.2.3", "1.3.0"),
            Some(Ordering::Less)
        );
        assert_eq!(
            ord().compare("semver", "15.4", "16.1"),
            Some(Ordering::Less)
        );

        // Unsupported schemes return None
        assert_eq!(ord().compare("unsupported", "1.0", "2.0"), None);
        assert_eq!(ord().compare("k8s", "1.28.0", "1.29.0"), None);
        assert_eq!(ord().compare("rpm", "1.0", "2.0"), None);
        assert_eq!(ord().compare("custom", "1.0", "2.0"), None);

        // SemVer prereleases and build metadata are rejected as non-numeric
        assert_eq!(ord().compare("semver", "1.0.0-alpha", "1.0.0"), None);
        assert_eq!(ord().compare("semver", "1.0.0", "1.0.0+build123"), None);

        // Predicate helper
        assert!(DottedNumericOrdering::supports_scheme("dotted"));
        assert!(DottedNumericOrdering::supports_scheme("dotted-numeric"));
        assert!(DottedNumericOrdering::supports_scheme("semver"));
        assert!(!DottedNumericOrdering::supports_scheme("k8s"));
        assert!(!DottedNumericOrdering::supports_scheme("custom"));
    }

    // --- VersionTransition Tests ---

    #[test]
    fn test_version_transition_upgrade() {
        let t = VersionTransition::new(
            "postgresql",
            VersionRef::known("dotted", "15.4"),
            VersionRef::known("dotted", "16.1"),
        );
        let o = ord();
        assert!(t.is_upgrade(&o));
        assert!(!t.is_downgrade(&o));
        assert!(!t.is_noop(&o));
    }

    #[test]
    fn test_version_transition_downgrade() {
        let t = VersionTransition::new(
            "postgresql",
            VersionRef::known("dotted", "16.1"),
            VersionRef::known("dotted", "15.4"),
        );
        let o = ord();
        assert!(!t.is_upgrade(&o));
        assert!(t.is_downgrade(&o));
        assert!(!t.is_noop(&o));
    }

    #[test]
    fn test_version_transition_noop() {
        let t = VersionTransition::new(
            "postgresql",
            VersionRef::known("dotted", "15.4"),
            VersionRef::known("dotted", "15.4.0"),
        );
        let o = ord();
        assert!(!t.is_upgrade(&o));
        assert!(!t.is_downgrade(&o));
        assert!(t.is_noop(&o));
    }

    #[test]
    fn test_version_transition_with_unknown_versions() {
        let o = ord();

        // Current unknown
        let t1 = VersionTransition::new(
            "postgresql",
            VersionRef::unknown("dotted"),
            VersionRef::known("dotted", "16.0"),
        );
        assert!(!t1.is_upgrade(&o));
        assert!(!t1.is_downgrade(&o));
        assert!(!t1.is_noop(&o));

        // Target unknown
        let t2 = VersionTransition::new(
            "postgresql",
            VersionRef::known("dotted", "15.0"),
            VersionRef::unknown("dotted"),
        );
        assert!(!t2.is_upgrade(&o));
        assert!(!t2.is_downgrade(&o));
        assert!(!t2.is_noop(&o));

        // Both unknown
        let t3 = VersionTransition::new(
            "postgresql",
            VersionRef::unknown("dotted"),
            VersionRef::unknown("dotted"),
        );
        assert!(!t3.is_upgrade(&o));
        assert!(!t3.is_downgrade(&o));
        assert!(!t3.is_noop(&o));
    }

    #[test]
    fn test_version_transition_with_inferred_versions() {
        let o = ord();
        let t = VersionTransition::new(
            "redis",
            VersionRef::inferred("dotted", "6.2.0"),
            VersionRef::known("dotted", "7.0.0"),
        );
        assert!(t.is_upgrade(&o));
        assert!(!t.is_downgrade(&o));
        assert!(!t.is_noop(&o));
        assert!(t.current.is_inferred());
        assert!(t.target.is_known());
    }

    #[test]
    fn test_version_transition_mismatched_schemes() {
        let o = ord();
        let t = VersionTransition::new(
            "kube-apiserver",
            VersionRef::known("dotted", "1.28"),
            VersionRef::known("semver", "1.29"),
        );
        assert!(!t.is_upgrade(&o));
        assert!(!t.is_downgrade(&o));
        assert!(!t.is_noop(&o));
    }

    #[test]
    fn test_version_transition_unsupported_schemes() {
        let o = ord();
        // Even if both endpoints use the same scheme, if the scheme is unsupported
        // by the comparator, transition predicates must safely evaluate to false.
        let t = VersionTransition::new(
            "custom-app",
            VersionRef::known("unsupported", "1.0"),
            VersionRef::known("unsupported", "2.0"),
        );
        assert!(!t.is_upgrade(&o));
        assert!(!t.is_downgrade(&o));
        assert!(!t.is_noop(&o));

        // Supported semver numeric transition works
        let t_semver = VersionTransition::new(
            "postgres",
            VersionRef::known("semver", "15.4"),
            VersionRef::known("semver", "16.1"),
        );
        assert!(t_semver.is_upgrade(&o));
        assert!(!t_semver.is_downgrade(&o));
        assert!(!t_semver.is_noop(&o));
    }

    #[test]
    fn test_version_transition_serde() {
        let t = VersionTransition::new(
            "postgresql",
            VersionRef::known("dotted", "15.4"),
            VersionRef::known("dotted", "16.1"),
        );
        let json = serde_json::to_string(&t).expect("serialize");
        let parsed: VersionTransition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, t);
    }
}
