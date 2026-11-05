//! WhatBreaks Compatibility Knowledge Evidence and Finding Status
//!
//! This module models *knowledge* evidence: documented claims about how a
//! component behaves across a version transition, such as an entry in an
//! official release note or upgrade guide. It also derives, for one proposed
//! version transition, how strongly a compatibility finding is supported by
//! that knowledge.
//!
//! # Why this is separate from `Evidence`
//!
//! [`Evidence`](crate::evidence::Evidence) is a factual, point-in-time
//! observation of a resource. It carries no inference and no confidence. A
//! statement in a release note is not an observation of any resource, so it is
//! modelled by [`KnowledgeEvidence`] and the two types never share a record.
//!
//! # Design Principles
//!
//! - **Provider-agnostic**: components and version schemes are open strings.
//!   Nothing here knows about Kubernetes, PostgreSQL or any other provider.
//!
//! - **Pure and deterministic**: no clock, no randomness, no I/O. Review dates,
//!   content hashes and IDs are supplied by the caller, and the same inputs
//!   always produce the same [`FindingStatus`].
//!
//! - **Conservative**: the absence of applicable evidence is
//!   [`FindingStatus::Unknown`], never "safe". A finding is only
//!   [`FindingStatus::Verified`] when an official source matches the exact
//!   transition and both versions are known.
//!
//! - **No numeric confidence**: support is expressed with enums only.
//!
//! - **Reference-only association**: [`FindingEvidenceStore`] stores IDs, never
//!   evidence contents, mirroring [`ProvenanceStore`](crate::provenance::ProvenanceStore).
//!
//! - **Provisional version types**: [`VersionKnowledge`], [`VersionRef`],
//!   [`VersionTransition`], [`VersionRange`] and [`VersionOrdering`] are small
//!   stand-ins. They are expected to be replaced by the version model from
//!   issue #18 (Version Transition Modeling) when it lands.
//!
//! # Finding Status
//!
//! ```text
//! applicable evidence?
//!   none ........................ Inferred (if an inference basis exists) / Unknown
//!   breaking AND non-breaking ... Conflicting
//!   one effect, both versions
//!     Known ..................... Verified
//!   one effect, a version
//!     only Inferred ............. Inferred
//! ```
//!
//! Evidence is *applicable* only when its source is official, its component
//! equals the transition's component, the transition is an upgrade (the target
//! is strictly newer than the current version under one scheme), and the
//! current and target versions fall inside the evidence's `applies_from` and
//! `applies_to` ranges. A downgrade or a no-op is never matched by an upgrade
//! claim, even when the two ranges overlap.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// VersionKnowledge
// ---------------------------------------------------------------------------

/// How well a version string is actually known.
///
/// An unknown version is represented explicitly with [`VersionKnowledge::Unknown`]
/// rather than by an absent field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VersionKnowledge {
    /// The version was stated or observed directly.
    Known,
    /// The version was derived indirectly (for example from an image tag).
    Inferred,
    /// The version is not known at all.
    Unknown,
}

impl VersionKnowledge {
    /// Stable string form, identical to the serialised form.
    pub fn as_str(&self) -> &'static str {
        match self {
            VersionKnowledge::Known => "KNOWN",
            VersionKnowledge::Inferred => "INFERRED",
            VersionKnowledge::Unknown => "UNKNOWN",
        }
    }

    /// `true` only for [`VersionKnowledge::Known`].
    pub fn is_known(&self) -> bool {
        matches!(self, VersionKnowledge::Known)
    }
}

// ---------------------------------------------------------------------------
// VersionRef
// ---------------------------------------------------------------------------

/// A version of a component, together with how well it is known.
///
/// `scheme` is an open string (for example `"semver"` or `"k8s"`), following the
/// same pattern as [`ObservationType`](crate::evidence::ObservationType). It
/// names the versioning scheme used to interpret `raw`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRef {
    /// The versioning scheme used to interpret `raw`.
    pub scheme: String,
    /// The version exactly as stated (empty when the version is unknown).
    pub raw: String,
    /// How well this version is known.
    pub knowledge: VersionKnowledge,
}

impl VersionRef {
    /// Create a version reference with explicit knowledge.
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

    /// A directly known version.
    pub fn known(scheme: impl Into<String>, raw: impl Into<String>) -> Self {
        Self::new(scheme, raw, VersionKnowledge::Known)
    }

    /// An indirectly inferred version.
    pub fn inferred(scheme: impl Into<String>, raw: impl Into<String>) -> Self {
        Self::new(scheme, raw, VersionKnowledge::Inferred)
    }

    /// An explicitly unknown version.
    pub fn unknown(scheme: impl Into<String>) -> Self {
        Self::new(scheme, "", VersionKnowledge::Unknown)
    }
}

// ---------------------------------------------------------------------------
// VersionTransition
// ---------------------------------------------------------------------------

/// A proposed move of one component from a current version to a target version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionTransition {
    /// The component being upgraded (an open string, for example `"postgresql"`).
    pub component: String,
    /// The version the component runs today.
    pub current: VersionRef,
    /// The version the component would move to.
    pub target: VersionRef,
}

impl VersionTransition {
    /// Create a transition.
    pub fn new(component: impl Into<String>, current: VersionRef, target: VersionRef) -> Self {
        Self {
            component: component.into(),
            current,
            target,
        }
    }

    /// Whether this transition moves to a strictly newer version.
    ///
    /// Returns `false` for a downgrade, a no-op (equal versions), versions in
    /// different schemes, and versions that cannot be compared.
    pub fn is_upgrade(&self, ord: &dyn VersionOrdering) -> bool {
        self.current.scheme == self.target.scheme
            && ord.compare(&self.current.scheme, &self.current.raw, &self.target.raw)
                == Some(Ordering::Less)
    }
}

// ---------------------------------------------------------------------------
// VersionOrdering
// ---------------------------------------------------------------------------

/// Scheme-aware comparison of two version strings.
///
/// Implementations return `None` when either version cannot be parsed or the
/// scheme is not supported. This lets issue #18 supply scheme-specific
/// comparison later without changing any caller.
pub trait VersionOrdering {
    /// Compare version `a` with version `b` under `scheme`.
    fn compare(&self, scheme: &str, a: &str, b: &str) -> Option<Ordering>;
}

/// Compares dot-separated numeric versions such as `1.2.10`.
///
/// It accepts any scheme string. Every dot-separated part must be a plain
/// non-negative integer; anything else (empty parts, signs, letters, a leading
/// `v`, overflow) makes the version unparsable. The shorter version is padded
/// with zeros, so `1.2` equals `1.2.0`, and parts compare numerically, so
/// `1.10` is greater than `1.9`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DottedNumericOrdering;

impl VersionOrdering for DottedNumericOrdering {
    fn compare(&self, _scheme: &str, a: &str, b: &str) -> Option<Ordering> {
        let left = parse_dotted(a)?;
        let right = parse_dotted(b)?;
        let len = left.len().max(right.len());
        for index in 0..len {
            let l = left.get(index).copied().unwrap_or(0);
            let r = right.get(index).copied().unwrap_or(0);
            match l.cmp(&r) {
                Ordering::Equal => continue,
                other => return Some(other),
            }
        }
        Some(Ordering::Equal)
    }
}

/// Parse `1.2.3` into numeric parts. Returns `None` for anything that is not
/// strictly digits separated by single dots.
fn parse_dotted(version: &str) -> Option<Vec<u64>> {
    if version.is_empty() {
        return None;
    }
    version
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// VersionRange
// ---------------------------------------------------------------------------

/// A range of versions within one scheme.
///
/// A missing bound is open on that side. Bounds are compared with a
/// [`VersionOrdering`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRange {
    /// The versioning scheme this range applies to.
    pub scheme: String,
    /// The lower bound, or `None` for no lower bound.
    pub lower: Option<String>,
    /// The upper bound, or `None` for no upper bound.
    pub upper: Option<String>,
    /// Whether a version equal to `lower` is inside the range.
    pub lower_inclusive: bool,
    /// Whether a version equal to `upper` is inside the range.
    pub upper_inclusive: bool,
}

impl VersionRange {
    /// Create a range with explicit bounds.
    pub fn new(
        scheme: impl Into<String>,
        lower: Option<String>,
        upper: Option<String>,
        lower_inclusive: bool,
        upper_inclusive: bool,
    ) -> Self {
        Self {
            scheme: scheme.into(),
            lower,
            upper,
            lower_inclusive,
            upper_inclusive,
        }
    }

    /// A range with no bounds: every known, parsable version of the scheme.
    pub fn unbounded(scheme: impl Into<String>) -> Self {
        Self::new(scheme, None, None, false, false)
    }

    /// Whether `version` falls inside this range.
    ///
    /// Returns `false` when the schemes differ, when the version's knowledge is
    /// [`VersionKnowledge::Unknown`], or when the version (or a bound) cannot be
    /// compared. An unparsable version never matches, not even an unbounded
    /// range. An [`VersionKnowledge::Inferred`] version can match; the status
    /// derivation caps what such a match is allowed to prove.
    pub fn contains(&self, version: &VersionRef, ord: &dyn VersionOrdering) -> bool {
        if self.scheme != version.scheme {
            return false;
        }
        if version.knowledge == VersionKnowledge::Unknown {
            return false;
        }
        // A version that cannot be compared with itself is unparsable.
        if ord
            .compare(&self.scheme, &version.raw, &version.raw)
            .is_none()
        {
            return false;
        }
        if let Some(lower) = &self.lower {
            match ord.compare(&self.scheme, &version.raw, lower) {
                Some(Ordering::Greater) => {}
                Some(Ordering::Equal) if self.lower_inclusive => {}
                _ => return false,
            }
        }
        if let Some(upper) = &self.upper {
            match ord.compare(&self.scheme, &version.raw, upper) {
                Some(Ordering::Less) => {}
                Some(Ordering::Equal) if self.upper_inclusive => {}
                _ => return false,
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// SourceAuthority / ClaimEffect
// ---------------------------------------------------------------------------

/// How authoritative the source of a claim is.
///
/// Only [`SourceAuthority::Official`] sources can make a finding verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceAuthority {
    /// Vendor or project documentation, such as official release notes.
    Official,
    /// Blog posts, forum answers and other non-official material.
    Community,
}

impl SourceAuthority {
    /// Stable string form, identical to the serialised form.
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceAuthority::Official => "OFFICIAL",
            SourceAuthority::Community => "COMMUNITY",
        }
    }
}

/// What a claim says about the transition it applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimEffect {
    /// The documented change can break consumers.
    Breaking,
    /// The documented change is backward compatible.
    NonBreaking,
}

impl ClaimEffect {
    /// Stable string form, identical to the serialised form.
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimEffect::Breaking => "BREAKING",
            ClaimEffect::NonBreaking => "NON_BREAKING",
        }
    }
}

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// The identity of one knowledge-evidence record.
///
/// The ID is supplied by the caller. Content-derived IDs belong to the
/// knowledge-pack loader, which is not part of this module.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KnowledgeEvidenceId(String);

impl KnowledgeEvidenceId {
    /// Wrap a caller-supplied identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KnowledgeEvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The identity of one compatibility finding.
///
/// The ID is supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FindingId(String);

impl FindingId {
    /// Wrap a caller-supplied identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// SourceRef
// ---------------------------------------------------------------------------

/// A reference to the document a claim was taken from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// Where the document can be found.
    pub url: String,
    /// A human-readable title for the document.
    pub title: String,
    /// The date the claim was last reviewed, as supplied by the caller. This
    /// module never reads a clock.
    pub reviewed_on: String,
    /// An opaque hash of the reviewed content, supplied by the caller.
    pub content_hash: String,
    /// How authoritative the source is.
    pub authority: SourceAuthority,
}

// ---------------------------------------------------------------------------
// KnowledgeEvidence
// ---------------------------------------------------------------------------

/// A documented claim about a component's behaviour across a version transition.
///
/// The claim applies when the transition's current version is inside
/// `applies_from` and its target version is inside `applies_to`.
///
/// A change introduced in release `V` that affects every upgrade crossing `V`,
/// including upgrades that span several releases, is expressed with open-ended
/// ranges: `applies_from` below `V` and `applies_to` from `V` upwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeEvidence {
    /// The identity of this record.
    pub id: KnowledgeEvidenceId,
    /// The component the claim is about.
    pub component: String,
    /// Versions the component moves *from* for which the claim holds.
    pub applies_from: VersionRange,
    /// Versions the component moves *to* for which the claim holds.
    pub applies_to: VersionRange,
    /// Whether the claim describes a breaking or a non-breaking change.
    pub effect: ClaimEffect,
    /// The extracted compatibility statement.
    pub claim: String,
    /// Where the claim comes from.
    pub source: SourceRef,
}

impl KnowledgeEvidence {
    /// Whether this evidence may be used to support a finding for `transition`.
    ///
    /// All of the following must hold: the source is official, the component
    /// matches, the transition is an upgrade (see [`VersionTransition::is_upgrade`]),
    /// and both versions fall inside the corresponding ranges. Requiring an
    /// upgrade stops overlapping ranges from matching a downgrade or a no-op.
    pub fn is_applicable_to(
        &self,
        transition: &VersionTransition,
        ord: &dyn VersionOrdering,
    ) -> bool {
        self.source.authority == SourceAuthority::Official
            && self.component == transition.component
            && transition.is_upgrade(ord)
            && self.applies_from.contains(&transition.current, ord)
            && self.applies_to.contains(&transition.target, ord)
    }
}

// ---------------------------------------------------------------------------
// FindingStatus
// ---------------------------------------------------------------------------

/// How strongly a compatibility finding is supported by knowledge evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingStatus {
    /// An official source matches the transition and both versions are known.
    Verified,
    /// Supported only indirectly: by an inference basis, or by a match that
    /// involves an inferred version.
    Inferred,
    /// Applicable official sources disagree about whether the change breaks.
    Conflicting,
    /// No applicable evidence and no inference basis.
    Unknown,
}

impl FindingStatus {
    /// Stable string form, identical to the serialised form.
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingStatus::Verified => "VERIFIED",
            FindingStatus::Inferred => "INFERRED",
            FindingStatus::Conflicting => "CONFLICTING",
            FindingStatus::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for FindingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Derive the status of a finding for `transition` from knowledge `evidence`.
///
/// `has_inference_basis` states whether the finding is otherwise supported by an
/// inference (for example a dependency observed in the graph). The function is
/// pure: the result depends only on its inputs and not on the order of
/// `evidence`.
///
/// Evidence is applicable only for an upgrade; see
/// [`KnowledgeEvidence::is_applicable_to`]. Rules, in this exact precedence:
///
/// 1. No applicable evidence: [`FindingStatus::Inferred`] if
///    `has_inference_basis`, otherwise [`FindingStatus::Unknown`].
/// 2. Applicable evidence contains both a breaking and a non-breaking claim:
///    [`FindingStatus::Conflicting`].
/// 3. Both transition versions are [`VersionKnowledge::Known`]:
///    [`FindingStatus::Verified`].
/// 4. Otherwise: [`FindingStatus::Inferred`].
pub fn derive_finding_status(
    transition: &VersionTransition,
    evidence: &[&KnowledgeEvidence],
    has_inference_basis: bool,
    ord: &dyn VersionOrdering,
) -> FindingStatus {
    let mut saw_breaking = false;
    let mut saw_non_breaking = false;

    for item in evidence {
        if !item.is_applicable_to(transition, ord) {
            continue;
        }
        match item.effect {
            ClaimEffect::Breaking => saw_breaking = true,
            ClaimEffect::NonBreaking => saw_non_breaking = true,
        }
    }

    if !saw_breaking && !saw_non_breaking {
        return if has_inference_basis {
            FindingStatus::Inferred
        } else {
            FindingStatus::Unknown
        };
    }
    if saw_breaking && saw_non_breaking {
        return FindingStatus::Conflicting;
    }
    if transition.current.knowledge.is_known() && transition.target.knowledge.is_known() {
        FindingStatus::Verified
    } else {
        FindingStatus::Inferred
    }
}

// ---------------------------------------------------------------------------
// FindingEvidenceStore
// ---------------------------------------------------------------------------

/// Deterministic comparator for [`FindingId`] values.
fn cmp_finding_id(a: &FindingId, b: &FindingId) -> Ordering {
    a.0.cmp(&b.0)
}

/// Deterministic comparator for [`KnowledgeEvidenceId`] values.
fn cmp_knowledge_evidence_id(a: &KnowledgeEvidenceId, b: &KnowledgeEvidenceId) -> Ordering {
    a.0.cmp(&b.0)
}

/// In-memory association between findings and the knowledge evidence behind them.
///
/// The store holds identifiers only; it never stores evidence contents. Two
/// indexes are kept in step:
///
/// - Forward: `FindingId -> HashSet<KnowledgeEvidenceId>`
/// - Reverse: `KnowledgeEvidenceId -> HashSet<FindingId>`
///
/// Internal storage uses hash collections for cheap mutation, and every public
/// query sorts on read so that output is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindingEvidenceStore {
    forward: HashMap<FindingId, HashSet<KnowledgeEvidenceId>>,
    reverse: HashMap<KnowledgeEvidenceId, HashSet<FindingId>>,
}

impl FindingEvidenceStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Associate `evidence` with `finding`.
    ///
    /// Returns `true` if the association is new and `false` if it already
    /// existed. The operation is idempotent.
    pub fn link(&mut self, finding: &FindingId, evidence: &KnowledgeEvidenceId) -> bool {
        let added = self
            .forward
            .entry(finding.clone())
            .or_default()
            .insert(evidence.clone());
        self.reverse
            .entry(evidence.clone())
            .or_default()
            .insert(finding.clone());
        added
    }

    /// Remove a finding and all of its associations.
    ///
    /// Evidence that no longer supports any finding is dropped from the reverse
    /// index. Returns `true` if the finding existed.
    pub fn remove_finding(&mut self, finding: &FindingId) -> bool {
        let evidence_ids = match self.forward.remove(finding) {
            Some(set) => set,
            None => return false,
        };
        for evidence_id in evidence_ids {
            let now_empty = match self.reverse.get_mut(&evidence_id) {
                Some(findings) => {
                    findings.remove(finding);
                    findings.is_empty()
                }
                None => false,
            };
            if now_empty {
                self.reverse.remove(&evidence_id);
            }
        }
        true
    }

    /// The evidence linked to `finding`, sorted. Empty for an unknown finding.
    pub fn evidence_for(&self, finding: &FindingId) -> Vec<KnowledgeEvidenceId> {
        match self.forward.get(finding) {
            Some(set) => {
                let mut list: Vec<KnowledgeEvidenceId> = set.iter().cloned().collect();
                list.sort_by(cmp_knowledge_evidence_id);
                list
            }
            None => Vec::new(),
        }
    }

    /// The findings that `evidence` supports, sorted. Empty for unknown evidence.
    pub fn findings_for(&self, evidence: &KnowledgeEvidenceId) -> Vec<FindingId> {
        match self.reverse.get(evidence) {
            Some(set) => {
                let mut list: Vec<FindingId> = set.iter().cloned().collect();
                list.sort_by(cmp_finding_id);
                list
            }
            None => Vec::new(),
        }
    }

    /// Whether `evidence` is linked to `finding`.
    pub fn contains_link(&self, finding: &FindingId, evidence: &KnowledgeEvidenceId) -> bool {
        self.forward
            .get(finding)
            .is_some_and(|set| set.contains(evidence))
    }

    /// Every finding that has at least one link, sorted.
    pub fn all_findings(&self) -> Vec<FindingId> {
        let mut list: Vec<FindingId> = self.forward.keys().cloned().collect();
        list.sort_by(cmp_finding_id);
        list
    }

    /// Every evidence ID that supports at least one finding, sorted.
    pub fn all_evidence_ids(&self) -> Vec<KnowledgeEvidenceId> {
        let mut list: Vec<KnowledgeEvidenceId> = self.reverse.keys().cloned().collect();
        list.sort_by(cmp_knowledge_evidence_id);
        list
    }

    /// The number of findings that have at least one link.
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    /// `true` when the store holds no links.
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// Verify that the forward and reverse indexes agree.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a description of the first violation found, checked in
    /// sorted order so the message is deterministic. A violation is an empty
    /// set, or a link present in one index but missing from the other.
    pub fn check_invariants(&self) -> Result<(), String> {
        let mut findings: Vec<&FindingId> = self.forward.keys().collect();
        findings.sort_by(|a, b| cmp_finding_id(a, b));
        for finding in findings {
            let linked = &self.forward[finding];
            if linked.is_empty() {
                return Err(format!("finding '{}' has an empty evidence set", finding));
            }
            let mut linked_sorted: Vec<&KnowledgeEvidenceId> = linked.iter().collect();
            linked_sorted.sort_by(|a, b| cmp_knowledge_evidence_id(a, b));
            for evidence in linked_sorted {
                let back = self.reverse.get(evidence);
                if !back.is_some_and(|set| set.contains(finding)) {
                    return Err(format!(
                        "finding '{}' links evidence '{}' but the reverse index does not",
                        finding, evidence
                    ));
                }
            }
        }

        let mut evidence_ids: Vec<&KnowledgeEvidenceId> = self.reverse.keys().collect();
        evidence_ids.sort_by(|a, b| cmp_knowledge_evidence_id(a, b));
        for evidence in evidence_ids {
            let supported = &self.reverse[evidence];
            if supported.is_empty() {
                return Err(format!("evidence '{}' has an empty finding set", evidence));
            }
            let mut supported_sorted: Vec<&FindingId> = supported.iter().collect();
            supported_sorted.sort_by(|a, b| cmp_finding_id(a, b));
            for finding in supported_sorted {
                let forward = self.forward.get(finding);
                if !forward.is_some_and(|set| set.contains(evidence)) {
                    return Err(format!(
                        "evidence '{}' supports finding '{}' but the forward index does not",
                        evidence, finding
                    ));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ------------------------------------------------------------

    fn ord() -> DottedNumericOrdering {
        DottedNumericOrdering
    }

    fn known(raw: &str) -> VersionRef {
        VersionRef::known("semver", raw)
    }

    fn range(lower: Option<&str>, upper: Option<&str>) -> VersionRange {
        VersionRange::new(
            "semver",
            lower.map(String::from),
            upper.map(String::from),
            true,
            false,
        )
    }

    fn transition(current: VersionRef, target: VersionRef) -> VersionTransition {
        VersionTransition::new("postgresql", current, target)
    }

    /// Evidence that applies to 14.0 <= from < 16.0 and 15.0 <= to < 18.0.
    fn evidence(id: &str, effect: ClaimEffect, authority: SourceAuthority) -> KnowledgeEvidence {
        KnowledgeEvidence {
            id: KnowledgeEvidenceId::new(id),
            component: "postgresql".to_string(),
            applies_from: range(Some("14.0"), Some("16.0")),
            applies_to: range(Some("15.0"), Some("18.0")),
            effect,
            claim: format!("claim {}", id),
            source: SourceRef {
                url: format!("https://example.test/{}", id),
                title: format!("Release notes {}", id),
                reviewed_on: "2026-01-01".to_string(),
                content_hash: format!("hash-{}", id),
                authority,
            },
        }
    }

    fn official(id: &str, effect: ClaimEffect) -> KnowledgeEvidence {
        evidence(id, effect, SourceAuthority::Official)
    }

    fn fid(id: &str) -> FindingId {
        FindingId::new(id)
    }

    fn eid(id: &str) -> KnowledgeEvidenceId {
        KnowledgeEvidenceId::new(id)
    }

    fn status(
        transition: &VersionTransition,
        items: &[&KnowledgeEvidence],
        basis: bool,
    ) -> FindingStatus {
        derive_finding_status(transition, items, basis, &ord())
    }

    // -----
    // 1. Dotted numeric comparison pads with zeros
    // -----
    #[test]
    fn test_dotted_ordering_pads_with_zeros() {
        assert_eq!(
            ord().compare("semver", "1.2", "1.2.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ord().compare("semver", "1.2.1", "1.2"),
            Some(Ordering::Greater)
        );
    }

    // -----
    // 2. Dotted numeric comparison is numeric, not lexical
    // -----
    #[test]
    fn test_dotted_ordering_is_numeric_not_lexical() {
        assert_eq!(
            ord().compare("semver", "1.10", "1.9"),
            Some(Ordering::Greater)
        );
        assert_eq!(ord().compare("semver", "2", "10"), Some(Ordering::Less));
    }

    // -----
    // 3. Dotted numeric comparison rejects non-numeric input
    // -----
    #[test]
    fn test_dotted_ordering_rejects_unparsable_versions() {
        for bad in ["", "1.x", "1..2", "+1", "-1", "v1.2", "1.2.", ".1", "1 2"] {
            assert_eq!(ord().compare("semver", bad, "1.0"), None, "left: {:?}", bad);
            assert_eq!(
                ord().compare("semver", "1.0", bad),
                None,
                "right: {:?}",
                bad
            );
        }
    }

    // -----
    // 4. Dotted numeric comparison rejects overflow
    // -----
    #[test]
    fn test_dotted_ordering_rejects_overflow() {
        assert_eq!(
            ord().compare("semver", "99999999999999999999999", "1"),
            None
        );
    }

    // -----
    // 5. Range bounds respect inclusive and exclusive flags
    // -----
    #[test]
    fn test_range_inclusive_and_exclusive_bounds() {
        let r = range(Some("14.0"), Some("16.0")); // [14.0, 16.0)
        assert!(r.contains(&known("14.0"), &ord()));
        assert!(r.contains(&known("15.9"), &ord()));
        assert!(!r.contains(&known("16.0"), &ord()));
        assert!(!r.contains(&known("13.9"), &ord()));

        let flipped = VersionRange::new(
            "semver",
            Some("14.0".to_string()),
            Some("16.0".to_string()),
            false,
            true,
        ); // (14.0, 16.0]
        assert!(!flipped.contains(&known("14.0"), &ord()));
        assert!(flipped.contains(&known("16.0"), &ord()));
    }

    // -----
    // 6. Open ends
    // -----
    #[test]
    fn test_range_open_ends() {
        let no_upper = range(Some("14.0"), None);
        assert!(no_upper.contains(&known("99.0"), &ord()));
        assert!(!no_upper.contains(&known("13.0"), &ord()));

        let no_lower = range(None, Some("16.0"));
        assert!(no_lower.contains(&known("0.1"), &ord()));
        assert!(!no_lower.contains(&known("16.0"), &ord()));

        let open = VersionRange::unbounded("semver");
        assert!(open.contains(&known("7.7.7"), &ord()));
    }

    // -----
    // 7. A mismatched scheme never matches
    // -----
    #[test]
    fn test_range_mismatched_scheme_is_false() {
        let r = range(Some("1.0"), None);
        assert!(!r.contains(&VersionRef::known("k8s", "1.5"), &ord()));
    }

    // -----
    // 8. An unparsable version never matches, even an unbounded range
    // -----
    #[test]
    fn test_range_unparsable_version_is_false() {
        let bounded = range(Some("1.0"), Some("2.0"));
        let open = VersionRange::unbounded("semver");
        assert!(!bounded.contains(&known("not-a-version"), &ord()));
        assert!(!open.contains(&known("not-a-version"), &ord()));
    }

    // -----
    // 9. An unknown version never matches
    // -----
    #[test]
    fn test_range_unknown_knowledge_is_false() {
        let open = VersionRange::unbounded("semver");
        assert!(!open.contains(&VersionRef::unknown("semver"), &ord()));
        // Even a parsable raw value cannot rescue an Unknown version.
        let sneaky = VersionRef::new("semver", "1.0", VersionKnowledge::Unknown);
        assert!(!open.contains(&sneaky, &ord()));
    }

    // -----
    // 10. An inferred version can match a range
    // -----
    #[test]
    fn test_range_inferred_knowledge_can_match() {
        let r = range(Some("1.0"), Some("2.0"));
        assert!(r.contains(&VersionRef::inferred("semver", "1.5"), &ord()));
    }

    // -----
    // 11. CRITICAL TEST 1: official evidence with known versions is verified
    // -----
    #[test]
    fn test_critical_1_verified_requires_official_match_and_known_versions() {
        let t = transition(known("15.4"), known("17.0"));
        let e = official("e1", ClaimEffect::Breaking);
        assert_eq!(status(&t, &[&e], false), FindingStatus::Verified);

        let non_breaking = official("e2", ClaimEffect::NonBreaking);
        assert_eq!(status(&t, &[&non_breaking], false), FindingStatus::Verified);
    }

    // -----
    // 12. CRITICAL TEST 2: disagreeing official sources are conflicting
    // -----
    #[test]
    fn test_critical_2_conflicting_sources() {
        let t = transition(known("15.4"), known("17.0"));
        let breaking = official("e1", ClaimEffect::Breaking);
        let non_breaking = official("e2", ClaimEffect::NonBreaking);
        assert_eq!(
            status(&t, &[&breaking, &non_breaking], false),
            FindingStatus::Conflicting
        );
        // Conflict wins over an inference basis.
        assert_eq!(
            status(&t, &[&breaking, &non_breaking], true),
            FindingStatus::Conflicting
        );
    }

    // -----
    // 13. CRITICAL TEST 3: no evidence and no basis is unknown
    // -----
    #[test]
    fn test_critical_3_no_evidence_is_unknown() {
        let t = transition(known("15.4"), known("17.0"));
        assert_eq!(status(&t, &[], false), FindingStatus::Unknown);
    }

    // -----
    // 14. CRITICAL TEST 4: no evidence but an inference basis is inferred
    // -----
    #[test]
    fn test_critical_4_inference_basis_is_inferred() {
        let t = transition(known("15.4"), known("17.0"));
        assert_eq!(status(&t, &[], true), FindingStatus::Inferred);
    }

    // -----
    // 15. An inferred version caps a match at inferred
    // -----
    #[test]
    fn test_inferred_version_caps_match_at_inferred() {
        let e = official("e1", ClaimEffect::Breaking);
        let inferred_current = transition(VersionRef::inferred("semver", "15.4"), known("17.0"));
        let inferred_target = transition(known("15.4"), VersionRef::inferred("semver", "17.0"));
        assert_eq!(
            status(&inferred_current, &[&e], false),
            FindingStatus::Inferred
        );
        assert_eq!(
            status(&inferred_target, &[&e], false),
            FindingStatus::Inferred
        );
    }

    // -----
    // 16. CRITICAL TEST 5: an unknown version never reaches verified
    // -----
    #[test]
    fn test_critical_5_unknown_version_never_verified() {
        let e = official("e1", ClaimEffect::Breaking);
        let unknown_current = transition(VersionRef::unknown("semver"), known("17.0"));
        let unknown_target = transition(known("15.4"), VersionRef::unknown("semver"));
        for t in [&unknown_current, &unknown_target] {
            assert_eq!(status(t, &[&e], false), FindingStatus::Unknown);
            assert_eq!(status(t, &[&e], true), FindingStatus::Inferred);
        }
    }

    // -----
    // 17. CRITICAL TEST 6: community sources never verify or conflict
    // -----
    #[test]
    fn test_critical_6_community_source_never_verifies() {
        let t = transition(known("15.4"), known("17.0"));
        let community = evidence("c1", ClaimEffect::Breaking, SourceAuthority::Community);
        assert_eq!(status(&t, &[&community], false), FindingStatus::Unknown);
        assert_eq!(status(&t, &[&community], true), FindingStatus::Inferred);

        // A community claim does not create a conflict with an official one.
        let official_breaking = official("e1", ClaimEffect::Breaking);
        let community_non_breaking =
            evidence("c2", ClaimEffect::NonBreaking, SourceAuthority::Community);
        assert_eq!(
            status(&t, &[&official_breaking, &community_non_breaking], false),
            FindingStatus::Verified
        );
    }

    // -----
    // 18. Evidence for another component is ignored
    // -----
    #[test]
    fn test_other_component_is_ignored() {
        let t = transition(known("15.4"), known("17.0"));
        let mut other = official("e1", ClaimEffect::Breaking);
        other.component = "redis".to_string();
        assert_eq!(status(&t, &[&other], false), FindingStatus::Unknown);
    }

    // -----
    // 19. Evidence whose ranges do not match the transition is ignored
    // -----
    #[test]
    fn test_out_of_range_evidence_is_ignored() {
        let e = official("e1", ClaimEffect::Breaking);
        let target_outside = transition(known("15.4"), known("18.0"));
        let current_outside = transition(known("13.9"), known("17.0"));
        assert_eq!(
            status(&target_outside, &[&e], false),
            FindingStatus::Unknown
        );
        assert_eq!(
            status(&current_outside, &[&e], false),
            FindingStatus::Unknown
        );
    }

    // -----
    // 20. Status does not depend on evidence order
    // -----
    #[test]
    fn test_status_is_independent_of_evidence_order() {
        let t = transition(known("15.4"), known("17.0"));
        let a = official("a", ClaimEffect::Breaking);
        let b = official("b", ClaimEffect::NonBreaking);
        let c = evidence("c", ClaimEffect::Breaking, SourceAuthority::Community);
        let orders: [[&KnowledgeEvidence; 3]; 4] =
            [[&a, &b, &c], [&c, &b, &a], [&b, &a, &c], [&b, &c, &a]];
        for order in orders {
            assert_eq!(status(&t, &order, false), FindingStatus::Conflicting);
        }
        // Repeating the call gives the same answer.
        assert_eq!(status(&t, &[&a], false), status(&t, &[&a], false));
    }

    // -----
    // 21. KnowledgeEvidence round-trips through JSON
    // -----
    #[test]
    fn test_knowledge_evidence_serialization_round_trip() {
        let e = official("e1", ClaimEffect::NonBreaking);
        let json = serde_json::to_string(&e).expect("serialize");
        let back: KnowledgeEvidence = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(e, back);
    }

    // -----
    // 22. Enums serialise to the same strings as as_str
    // -----
    #[test]
    fn test_enums_serialize_to_as_str() {
        fn json<T: Serialize>(value: &T) -> String {
            serde_json::to_string(value).expect("serialize")
        }
        assert_eq!(json(&ClaimEffect::NonBreaking), "\"NON_BREAKING\"");
        assert_eq!(json(&ClaimEffect::Breaking), "\"BREAKING\"");
        assert_eq!(json(&SourceAuthority::Official), "\"OFFICIAL\"");
        assert_eq!(json(&SourceAuthority::Community), "\"COMMUNITY\"");
        for k in [
            VersionKnowledge::Known,
            VersionKnowledge::Inferred,
            VersionKnowledge::Unknown,
        ] {
            assert_eq!(json(&k), format!("\"{}\"", k.as_str()));
        }
        for s in [
            FindingStatus::Verified,
            FindingStatus::Inferred,
            FindingStatus::Conflicting,
            FindingStatus::Unknown,
        ] {
            assert_eq!(json(&s), format!("\"{}\"", s.as_str()));
            assert_eq!(s.to_string(), s.as_str());
        }
    }

    // -----
    // 23. IDs serialise transparently as plain strings
    // -----
    #[test]
    fn test_ids_serialize_transparently() {
        assert_eq!(
            serde_json::to_string(&fid("f1")).expect("serialize"),
            "\"f1\""
        );
        assert_eq!(
            serde_json::to_string(&eid("e1")).expect("serialize"),
            "\"e1\""
        );
        assert_eq!(fid("f1").as_str(), "f1");
        assert_eq!(eid("e1").to_string(), "e1");
    }

    // -----
    // 24. A new store is empty
    // -----
    #[test]
    fn test_new_store_is_empty() {
        let store = FindingEvidenceStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.all_findings().is_empty());
        assert!(store.all_evidence_ids().is_empty());
        assert!(store.check_invariants().is_ok());
    }

    // -----
    // 25. Linking is idempotent
    // -----
    #[test]
    fn test_link_is_idempotent() {
        let mut store = FindingEvidenceStore::new();
        assert!(store.link(&fid("f1"), &eid("e1")));
        assert!(!store.link(&fid("f1"), &eid("e1")));
        assert_eq!(store.len(), 1);
        assert_eq!(store.evidence_for(&fid("f1")), vec![eid("e1")]);
        assert!(store.contains_link(&fid("f1"), &eid("e1")));
        assert!(!store.contains_link(&fid("f1"), &eid("e2")));
    }

    // -----
    // 26. CRITICAL TEST 7: forward and reverse lookups agree
    // -----
    #[test]
    fn test_critical_7_forward_and_reverse_lookups_agree() {
        let mut store = FindingEvidenceStore::new();
        store.link(&fid("f1"), &eid("e1"));
        store.link(&fid("f1"), &eid("e2"));
        store.link(&fid("f2"), &eid("e2"));

        assert_eq!(store.evidence_for(&fid("f1")), vec![eid("e1"), eid("e2")]);
        assert_eq!(store.evidence_for(&fid("f2")), vec![eid("e2")]);
        assert_eq!(store.findings_for(&eid("e2")), vec![fid("f1"), fid("f2")]);
        assert_eq!(store.findings_for(&eid("e1")), vec![fid("f1")]);
        assert!(store.check_invariants().is_ok());
    }

    // -----
    // 27. Query output is sorted regardless of insertion order
    // -----
    #[test]
    fn test_query_output_is_sorted() {
        let mut store = FindingEvidenceStore::new();
        for id in ["e9", "e3", "e7", "e1", "e5"] {
            store.link(&fid("f1"), &eid(id));
        }
        for id in ["f9", "f3", "f7"] {
            store.link(&fid(id), &eid("e1"));
        }
        let evidence: Vec<String> = store
            .evidence_for(&fid("f1"))
            .iter()
            .map(|e| e.to_string())
            .collect();
        assert_eq!(evidence, vec!["e1", "e3", "e5", "e7", "e9"]);
        let findings: Vec<String> = store
            .findings_for(&eid("e1"))
            .iter()
            .map(|f| f.to_string())
            .collect();
        assert_eq!(findings, vec!["f1", "f3", "f7", "f9"]);
        assert_eq!(store.all_findings().len(), 4);
        assert_eq!(store.all_evidence_ids().len(), 5);
    }

    // -----
    // 28. Unknown lookups return empty results
    // -----
    #[test]
    fn test_unknown_lookups_are_empty() {
        let store = FindingEvidenceStore::new();
        assert!(store.evidence_for(&fid("nope")).is_empty());
        assert!(store.findings_for(&eid("nope")).is_empty());
        assert!(!store.contains_link(&fid("nope"), &eid("nope")));
    }

    // -----
    // 29. Removing a finding cleans both indexes
    // -----
    #[test]
    fn test_remove_finding_cleans_both_indexes() {
        let mut store = FindingEvidenceStore::new();
        store.link(&fid("f1"), &eid("e1"));
        store.link(&fid("f1"), &eid("e2"));
        store.link(&fid("f2"), &eid("e2"));

        assert!(store.remove_finding(&fid("f1")));
        assert!(!store.remove_finding(&fid("f1")));
        assert!(store.evidence_for(&fid("f1")).is_empty());
        // e1 only supported f1, so it is gone; e2 still supports f2.
        assert!(store.findings_for(&eid("e1")).is_empty());
        assert_eq!(store.findings_for(&eid("e2")), vec![fid("f2")]);
        assert_eq!(store.all_evidence_ids(), vec![eid("e2")]);
        assert!(store.check_invariants().is_ok());
    }

    // -----
    // 30. Re-adding after removal works
    // -----
    #[test]
    fn test_repeated_link_and_remove() {
        let mut store = FindingEvidenceStore::new();
        for _ in 0..5 {
            assert!(store.link(&fid("f1"), &eid("e1")));
            assert!(store.remove_finding(&fid("f1")));
            assert!(store.is_empty());
            assert!(store.check_invariants().is_ok());
        }
    }

    // -----
    // 31. check_invariants detects a forward-only link
    // -----
    #[test]
    fn test_check_invariants_detects_missing_reverse_entry() {
        let mut store = FindingEvidenceStore::new();
        store.link(&fid("f1"), &eid("e1"));
        store
            .forward
            .entry(fid("f1"))
            .or_default()
            .insert(eid("e2")); // no reverse entry for e2
        let err = store.check_invariants().unwrap_err();
        assert!(err.contains("reverse index"), "unexpected message: {}", err);
    }

    // -----
    // 32. check_invariants detects a reverse-only link
    // -----
    #[test]
    fn test_check_invariants_detects_missing_forward_entry() {
        let mut store = FindingEvidenceStore::new();
        store.link(&fid("f1"), &eid("e1"));
        store
            .reverse
            .entry(eid("e1"))
            .or_default()
            .insert(fid("f2")); // no forward entry for f2
        let err = store.check_invariants().unwrap_err();
        assert!(err.contains("forward index"), "unexpected message: {}", err);
    }

    // -----
    // 33. check_invariants detects empty sets
    // -----
    #[test]
    fn test_check_invariants_detects_empty_sets() {
        let mut forward_empty = FindingEvidenceStore::new();
        forward_empty.forward.insert(fid("f1"), HashSet::new());
        assert!(forward_empty.check_invariants().is_err());

        let mut reverse_empty = FindingEvidenceStore::new();
        reverse_empty.reverse.insert(eid("e1"), HashSet::new());
        assert!(reverse_empty.check_invariants().is_err());
    }

    // -----
    // 34. The store is Send and Sync
    // -----
    #[test]
    fn test_send_sync_thread_safety() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FindingEvidenceStore>();
        assert_send_sync::<KnowledgeEvidence>();
        assert_send_sync::<VersionTransition>();
        assert_send_sync::<DottedNumericOrdering>();
    }

    // -----
    // 35. CRITICAL TEST 8: a change from an intermediate release applies to a spanning upgrade
    // -----
    #[test]
    fn test_critical_8_intermediate_release_applies_to_spanning_upgrade() {
        // "Introduced in 16.0": affects any upgrade that starts before 16.0 and
        // ends at or after 16.0, expressed with open-ended ranges.
        let mut crossing = official("e1", ClaimEffect::Breaking);
        crossing.applies_from = range(None, Some("16.0")); // below 16.0
        crossing.applies_to = range(Some("16.0"), None); // 16.0 and above

        // 15.4 -> 17.0 spans 16.0, so the claim applies.
        let spanning = transition(known("15.4"), known("17.0"));
        assert_eq!(
            status(&spanning, &[&crossing], false),
            FindingStatus::Verified
        );

        // 15.9 -> 16.0 reaches the release exactly, so the claim applies.
        let reaching = transition(known("15.9"), known("16.0"));
        assert_eq!(
            status(&reaching, &[&crossing], false),
            FindingStatus::Verified
        );

        // 15.4 -> 15.9 stays below 16.0, so the claim does not apply.
        let before = transition(known("15.4"), known("15.9"));
        assert_eq!(status(&before, &[&crossing], false), FindingStatus::Unknown);

        // 16.1 -> 17.0 starts after 16.0, so the claim does not apply.
        let after = transition(known("16.1"), known("17.0"));
        assert_eq!(status(&after, &[&crossing], false), FindingStatus::Unknown);
    }

    // -----
    // 36. CRITICAL TEST 9: a downgrade or no-op never matches an upgrade claim
    // -----
    #[test]
    fn test_critical_9_downgrade_and_noop_never_match_upgrade_claim() {
        // The helper's ranges overlap: applies_from is [14.0, 16.0) and
        // applies_to is [15.0, 18.0), so 15.5 and 15.0 sit inside both.
        let e = official("e1", ClaimEffect::Breaking);

        let downgrade = transition(known("15.5"), known("15.0"));
        let noop = transition(known("15.0"), known("15.0"));
        let padded_noop = transition(known("15.0"), known("15.0.0"));
        for t in [&downgrade, &noop, &padded_noop] {
            assert_eq!(status(t, &[&e], false), FindingStatus::Unknown);
            assert_eq!(status(t, &[&e], true), FindingStatus::Inferred);
        }

        // A real upgrade inside the overlap is still verified.
        let upgrade = transition(known("15.0"), known("15.5"));
        assert_eq!(status(&upgrade, &[&e], false), FindingStatus::Verified);
    }

    // -----
    // 37. is_upgrade accepts only a strictly newer target under one scheme
    // -----
    #[test]
    fn test_is_upgrade_rules() {
        assert!(transition(known("1.9"), known("1.10")).is_upgrade(&ord()));
        assert!(transition(known("1"), known("1.0.1")).is_upgrade(&ord()));

        assert!(!transition(known("1.10"), known("1.9")).is_upgrade(&ord()));
        assert!(!transition(known("1.2"), known("1.2.0")).is_upgrade(&ord()));
        assert!(!transition(known("1.2"), known("1.2")).is_upgrade(&ord()));

        // Different schemes cannot be compared.
        let mixed = transition(known("1.0"), VersionRef::known("k8s", "2.0"));
        assert!(!mixed.is_upgrade(&ord()));

        // An unparsable or unknown version cannot be compared either.
        assert!(!transition(known("not-a-version"), known("2.0")).is_upgrade(&ord()));
        assert!(!transition(VersionRef::unknown("semver"), known("2.0")).is_upgrade(&ord()));
    }
}
