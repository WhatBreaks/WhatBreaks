//! WhatBreaks Diagnostic and Analysis Completeness Model
//!
//! Provides structured, machine-readable diagnostics for unknown resources,
//! unresolved references, unmapped runtime connections, and collection scope integrity.
//!
//! # Architecture & Invariants
//!
//! - **Analysis Completeness**: `is_complete == (status == CompletenessStatus::Complete)`.
//! - **Relevance Partitioning**: Diagnostics distinguish `Target`, `BlastRadius`, and `Workspace` relevance.
//! - **Determinism**: Diagnostics and reasons are deterministically ordered.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::evidence::EvidenceId;
use crate::resource::ResourceIdentity;

// ---------------------------------------------------------------------------
// Diagnostic Code Constants
// ---------------------------------------------------------------------------

pub mod diagnostic_code {
    /// The target resource requested for analysis has not been directly observed.
    pub const UNOBSERVED_TARGET: &str = "UNOBSERVED_TARGET";

    /// A relationship points to an endpoint resource that has not been directly observed.
    pub const UNRESOLVED_REFERENCE: &str = "UNRESOLVED_REFERENCE";

    /// Runtime connection evidence exists but could not be mapped to any known target.
    pub const UNMAPPED_CONNECTION: &str = "UNMAPPED_CONNECTION";

    /// A required or optional collection scope failed or was incomplete.
    pub const INCOMPLETE_SCOPE: &str = "INCOMPLETE_SCOPE";

    /// Contradictory evidence observations prevented deterministic relationship derivation.
    pub const DISCOVERY_CONFLICT: &str = "DISCOVERY_CONFLICT";
}

// ---------------------------------------------------------------------------
// Diagnostic Severity
// ---------------------------------------------------------------------------

/// Severity level of a diagnostic finding.
///
/// Ordered by an explicit [`rank`](Self::rank) (`Info < Warning < Error`), so
/// reordering or inserting variants can't silently change how diagnostics sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl DiagnosticSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

impl DiagnosticSeverity {
    /// Sort rank used by `Ord`; lower sorts first.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Error => 2,
        }
    }
}

impl PartialOrd for DiagnosticSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DiagnosticSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Diagnostic Relevance
// ---------------------------------------------------------------------------

/// Scope of relevance for a diagnostic in relation to an analysis query.
///
/// Ordered by an explicit [`rank`](Self::rank) (`Target < BlastRadius < Workspace`),
/// independent of the variant declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticRelevance {
    /// Directly affects the resource being analyzed.
    Target,
    /// Affects a dependency or candidate impact path in the blast radius.
    BlastRadius,
    /// Describes workspace-wide collection conditions not necessarily impacting the query.
    Workspace,
}

impl DiagnosticRelevance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Target => "TARGET",
            Self::BlastRadius => "BLAST_RADIUS",
            Self::Workspace => "WORKSPACE",
        }
    }
}

impl DiagnosticRelevance {
    /// Sort rank used by `Ord`; lower sorts first.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Target => 0,
            Self::BlastRadius => 1,
            Self::Workspace => 2,
        }
    }
}

impl PartialOrd for DiagnosticRelevance {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DiagnosticRelevance {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl fmt::Display for DiagnosticRelevance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Completeness Status
// ---------------------------------------------------------------------------

/// High-level completeness classification of an impact analysis result.
///
/// Ordered by an explicit [`rank`](Self::rank) (`Complete < Partial < Degraded`),
/// independent of the variant declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompletenessStatus {
    /// Analysis has complete observation coverage for the target and blast radius.
    Complete,
    /// Relevant observations or dependencies are unresolved, but partial blast radius is available.
    Partial,
    /// Critical observations or targets are missing; results are degraded.
    Degraded,
}

impl CompletenessStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "COMPLETE",
            Self::Partial => "PARTIAL",
            Self::Degraded => "DEGRADED",
        }
    }
}

impl CompletenessStatus {
    /// Sort rank used by `Ord`; lower sorts first.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::Partial => 1,
            Self::Degraded => 2,
        }
    }
}

impl PartialOrd for CompletenessStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompletenessStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl fmt::Display for CompletenessStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Analysis Completeness
// ---------------------------------------------------------------------------

/// Structured completeness metadata detailing whether the blast radius is authoritative.
///
/// Deserialization checks the `is_complete` invariant, so JSON like
/// `{"is_complete": true, "status": "PARTIAL"}` is rejected instead of
/// producing an inconsistent value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AnalysisCompletenessWire")]
pub struct AnalysisCompleteness {
    /// Invariant: `is_complete == (status == CompletenessStatus::Complete)`.
    pub is_complete: bool,
    /// Completeness status classification.
    pub status: CompletenessStatus,
    /// Factual reasons explaining any partial or degraded completeness.
    pub reasons: Vec<String>,
}

/// Unvalidated wire shape of [`AnalysisCompleteness`], checked in `TryFrom`.
#[derive(Deserialize)]
struct AnalysisCompletenessWire {
    is_complete: bool,
    status: CompletenessStatus,
    reasons: Vec<String>,
}

impl TryFrom<AnalysisCompletenessWire> for AnalysisCompleteness {
    type Error = String;

    fn try_from(wire: AnalysisCompletenessWire) -> Result<Self, Self::Error> {
        let expected = wire.status == CompletenessStatus::Complete;
        if wire.is_complete != expected {
            return Err(format!(
                "is_complete is {} but status {} requires {}",
                wire.is_complete, wire.status, expected
            ));
        }
        Ok(Self::new(wire.status, wire.reasons))
    }
}

impl AnalysisCompleteness {
    /// Construct a new `AnalysisCompleteness`, enforcing the invariant:
    /// `is_complete == (status == CompletenessStatus::Complete)`.
    pub fn new(status: CompletenessStatus, reasons: Vec<String>) -> Self {
        Self {
            is_complete: status == CompletenessStatus::Complete,
            status,
            reasons,
        }
    }

    /// Construct a complete status with no degradation reasons.
    pub fn complete() -> Self {
        Self::new(CompletenessStatus::Complete, Vec::new())
    }

    /// Construct a partial status with explanatory reasons.
    pub fn partial(reasons: Vec<String>) -> Self {
        Self::new(CompletenessStatus::Partial, reasons)
    }

    /// Construct a degraded status with explanatory reasons.
    pub fn degraded(reasons: Vec<String>) -> Self {
        Self::new(CompletenessStatus::Degraded, reasons)
    }
}

// ---------------------------------------------------------------------------
// Diagnostic Item
// ---------------------------------------------------------------------------

/// Machine-readable diagnostic item detailing dependency, connection, or scope findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable machine-readable code (e.g. `UNOBSERVED_TARGET`, `UNRESOLVED_REFERENCE`).
    pub code: String,
    /// Severity level (`INFO`, `WARNING`, `ERROR`).
    pub severity: DiagnosticSeverity,
    /// Human-readable explanation.
    pub message: String,
    /// The subject resource primarily associated with this diagnostic.
    pub target: Option<ResourceIdentity>,
    /// Referenced resource endpoint if this diagnostic concerns an unresolved reference.
    pub referenced_resource: Option<ResourceIdentity>,
    /// Unresolved network endpoint (e.g. `10.0.0.1:8080`) if unmapped connection.
    pub unresolved_endpoint: Option<String>,
    /// Collection scope identifier (e.g. `pods`, `ingresses`, `cluster/prod`).
    pub scope: Option<String>,
    /// IDs of supporting evidence observations supporting this diagnostic.
    pub supporting_evidence_ids: Vec<EvidenceId>,
    /// Relevance level to the query (`TARGET`, `BLAST_RADIUS`, `WORKSPACE`).
    pub relevance: DiagnosticRelevance,
    /// Completeness impact classification.
    pub completeness_impact: CompletenessStatus,
    /// Timestamp when first observed (RFC 3339 string).
    pub first_observed_at: Option<String>,
    /// Timestamp when last observed (RFC 3339 string).
    pub last_observed_at: Option<String>,
}

impl Diagnostic {
    /// Create a new Diagnostic.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
        target: Option<ResourceIdentity>,
        referenced_resource: Option<ResourceIdentity>,
        unresolved_endpoint: Option<String>,
        scope: Option<String>,
        supporting_evidence_ids: Vec<EvidenceId>,
        relevance: DiagnosticRelevance,
        completeness_impact: CompletenessStatus,
        first_observed_at: Option<String>,
        last_observed_at: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            target,
            referenced_resource,
            unresolved_endpoint,
            scope,
            supporting_evidence_ids,
            relevance,
            completeness_impact,
            first_observed_at,
            last_observed_at,
        }
    }

    /// Create an unobserved target diagnostic.
    pub fn unobserved_target(target: ResourceIdentity) -> Self {
        let msg = format!(
            "Target resource '{}:{}:{}' has not been directly observed in authoritative state",
            target.provider, target.resource_type, target.provider_id
        );
        Self {
            code: diagnostic_code::UNOBSERVED_TARGET.to_string(),
            severity: DiagnosticSeverity::Error,
            message: msg,
            target: Some(target),
            referenced_resource: None,
            unresolved_endpoint: None,
            scope: None,
            supporting_evidence_ids: Vec::new(),
            relevance: DiagnosticRelevance::Target,
            completeness_impact: CompletenessStatus::Degraded,
            first_observed_at: None,
            last_observed_at: None,
        }
    }

    /// Natural comparison key for deterministic ordering.
    ///
    /// This is a partial key: it compares code, message, target provider id and
    /// referenced resource provider id only. Diagnostics that match on those but
    /// differ in severity, relevance, completeness impact, endpoint, scope or
    /// evidence compare equal, so with a stable sort they keep their input order.
    pub fn sort_key(&self) -> (&str, &str, &str, &str) {
        let target_str = match &self.target {
            Some(t) => t.provider_id.as_str(),
            None => "",
        };
        let ref_str = match &self.referenced_resource {
            Some(r) => r.provider_id.as_str(),
            None => "",
        };
        (
            self.code.as_str(),
            self.message.as_str(),
            target_str,
            ref_str,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{Provider, ResourceKind};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn k8s_service(id: &str) -> ResourceIdentity {
        ResourceIdentity::new(
            Provider::new("kubernetes"),
            ResourceKind::new("service"),
            id,
        )
    }

    fn diag(
        code: &str,
        message: &str,
        target: Option<&str>,
        reference: Option<&str>,
    ) -> Diagnostic {
        Diagnostic::new(
            code,
            DiagnosticSeverity::Warning,
            message,
            target.map(k8s_service),
            reference.map(k8s_service),
            None,
            None,
            Vec::new(),
            DiagnosticRelevance::BlastRadius,
            CompletenessStatus::Partial,
            None,
            None,
        )
    }

    // -----------------------------------------------------------------------
    // AnalysisCompleteness
    // -----------------------------------------------------------------------

    #[test]
    fn complete_has_no_reasons_and_is_complete() {
        let c = AnalysisCompleteness::complete();
        assert_eq!(c.status, CompletenessStatus::Complete);
        assert!(c.is_complete);
        assert!(c.reasons.is_empty());
    }

    #[test]
    fn partial_and_degraded_keep_reasons_and_are_not_complete() {
        let reasons = vec![
            "pods scope failed".to_string(),
            "ingress unresolved".to_string(),
        ];

        let partial = AnalysisCompleteness::partial(reasons.clone());
        assert_eq!(partial.status, CompletenessStatus::Partial);
        assert!(!partial.is_complete);
        assert_eq!(partial.reasons, reasons);

        let degraded = AnalysisCompleteness::degraded(reasons.clone());
        assert_eq!(degraded.status, CompletenessStatus::Degraded);
        assert!(!degraded.is_complete);
        assert_eq!(degraded.reasons, reasons);
    }

    #[test]
    fn new_keeps_is_complete_in_sync_with_status() {
        for status in [
            CompletenessStatus::Complete,
            CompletenessStatus::Partial,
            CompletenessStatus::Degraded,
        ] {
            let c = AnalysisCompleteness::new(status, vec!["reason".to_string()]);
            assert_eq!(
                c.is_complete,
                status == CompletenessStatus::Complete,
                "{status}"
            );
            assert_eq!(c.status, status);
        }
    }

    // -----------------------------------------------------------------------
    // Enum string forms and ordering
    // -----------------------------------------------------------------------

    #[test]
    fn severity_as_str_and_display_match() {
        for (severity, expected) in [
            (DiagnosticSeverity::Info, "INFO"),
            (DiagnosticSeverity::Warning, "WARNING"),
            (DiagnosticSeverity::Error, "ERROR"),
        ] {
            assert_eq!(severity.as_str(), expected);
            assert_eq!(severity.to_string(), expected);
        }
    }

    #[test]
    fn relevance_as_str_and_display_match() {
        for (relevance, expected) in [
            (DiagnosticRelevance::Target, "TARGET"),
            (DiagnosticRelevance::BlastRadius, "BLAST_RADIUS"),
            (DiagnosticRelevance::Workspace, "WORKSPACE"),
        ] {
            assert_eq!(relevance.as_str(), expected);
            assert_eq!(relevance.to_string(), expected);
        }
    }

    #[test]
    fn completeness_status_as_str_and_display_match() {
        for (status, expected) in [
            (CompletenessStatus::Complete, "COMPLETE"),
            (CompletenessStatus::Partial, "PARTIAL"),
            (CompletenessStatus::Degraded, "DEGRADED"),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(status.to_string(), expected);
        }
    }

    #[test]
    fn ordering_follows_explicit_rank() {
        assert!(DiagnosticSeverity::Info < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Error);
        assert!(DiagnosticRelevance::Target < DiagnosticRelevance::BlastRadius);
        assert!(DiagnosticRelevance::BlastRadius < DiagnosticRelevance::Workspace);
        assert!(CompletenessStatus::Complete < CompletenessStatus::Partial);
        assert!(CompletenessStatus::Partial < CompletenessStatus::Degraded);

        let mut severities = [
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Info,
            DiagnosticSeverity::Warning,
        ];
        severities.sort();
        assert_eq!(
            severities.iter().map(|s| s.rank()).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn rank_is_unique_per_variant() {
        let severity = [
            DiagnosticSeverity::Info,
            DiagnosticSeverity::Warning,
            DiagnosticSeverity::Error,
        ]
        .map(|v| v.rank());
        let relevance = [
            DiagnosticRelevance::Target,
            DiagnosticRelevance::BlastRadius,
            DiagnosticRelevance::Workspace,
        ]
        .map(|v| v.rank());
        let status = [
            CompletenessStatus::Complete,
            CompletenessStatus::Partial,
            CompletenessStatus::Degraded,
        ]
        .map(|v| v.rank());
        for ranks in [severity, relevance, status] {
            let unique: std::collections::BTreeSet<_> = ranks.iter().collect();
            assert_eq!(unique.len(), ranks.len());
        }
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    #[test]
    fn enums_serialize_to_the_same_strings_as_as_str() {
        let pairs = [
            (
                serde_json::to_value(DiagnosticSeverity::Warning).unwrap(),
                DiagnosticSeverity::Warning.as_str(),
            ),
            (
                serde_json::to_value(DiagnosticRelevance::BlastRadius).unwrap(),
                DiagnosticRelevance::BlastRadius.as_str(),
            ),
            (
                serde_json::to_value(CompletenessStatus::Degraded).unwrap(),
                CompletenessStatus::Degraded.as_str(),
            ),
        ];
        for (json, expected) in pairs {
            assert_eq!(json, serde_json::Value::String(expected.to_string()));
        }
    }

    #[test]
    fn enums_round_trip_through_json() {
        let severity: DiagnosticSeverity = serde_json::from_str("\"ERROR\"").unwrap();
        assert_eq!(severity, DiagnosticSeverity::Error);
        let relevance: DiagnosticRelevance = serde_json::from_str("\"BLAST_RADIUS\"").unwrap();
        assert_eq!(relevance, DiagnosticRelevance::BlastRadius);
        let status: CompletenessStatus = serde_json::from_str("\"PARTIAL\"").unwrap();
        assert_eq!(status, CompletenessStatus::Partial);

        assert!(serde_json::from_str::<DiagnosticSeverity>("\"error\"").is_err());
    }

    #[test]
    fn analysis_completeness_round_trips() {
        let c = AnalysisCompleteness::partial(vec!["ingress scope timed out".to_string()]);
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["is_complete"], false);
        assert_eq!(json["status"], "PARTIAL");
        assert_eq!(json["reasons"][0], "ingress scope timed out");

        let back: AnalysisCompleteness = serde_json::from_value(json).unwrap();
        assert_eq!(back, c);
        assert_eq!(
            back.is_complete,
            back.status == CompletenessStatus::Complete
        );
    }

    #[test]
    fn analysis_completeness_rejects_json_that_breaks_the_invariant() {
        for (is_complete, status) in [(true, "PARTIAL"), (true, "DEGRADED"), (false, "COMPLETE")] {
            let json = serde_json::json!({
                "is_complete": is_complete,
                "status": status,
                "reasons": [],
            });
            let err = serde_json::from_value::<AnalysisCompleteness>(json)
                .expect_err("inconsistent completeness must not deserialize");
            assert!(err.to_string().contains("is_complete"), "{err}");
        }
    }

    #[test]
    fn analysis_completeness_accepts_consistent_json() {
        let json = serde_json::json!({
            "is_complete": true,
            "status": "COMPLETE",
            "reasons": [],
        });
        let c: AnalysisCompleteness = serde_json::from_value(json).unwrap();
        assert_eq!(c, AnalysisCompleteness::complete());
    }

    #[test]
    fn diagnostic_round_trips_with_all_fields_set() {
        let d = Diagnostic::new(
            diagnostic_code::UNMAPPED_CONNECTION,
            DiagnosticSeverity::Info,
            "connection to 10.0.0.1:8080 could not be mapped",
            Some(k8s_service("payments/api")),
            Some(k8s_service("payments/db")),
            Some("10.0.0.1:8080".to_string()),
            Some("pods".to_string()),
            vec![EvidenceId::new(), EvidenceId::new()],
            DiagnosticRelevance::Workspace,
            CompletenessStatus::Partial,
            Some("2026-10-01T10:00:00Z".to_string()),
            Some("2026-10-02T10:00:00Z".to_string()),
        );

        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["code"], "UNMAPPED_CONNECTION");
        assert_eq!(json["severity"], "INFO");
        assert_eq!(json["relevance"], "WORKSPACE");
        assert_eq!(json["completeness_impact"], "PARTIAL");

        let back: Diagnostic = serde_json::from_value(json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn diagnostic_without_optional_fields_deserializes_with_none() {
        // A minimal payload, as an older or partial producer might send it:
        // every Option field is absent rather than null.
        let json = serde_json::json!({
            "code": "INCOMPLETE_SCOPE",
            "severity": "WARNING",
            "message": "pods scope failed",
            "supporting_evidence_ids": [],
            "relevance": "WORKSPACE",
            "completeness_impact": "PARTIAL",
        });

        let d: Diagnostic = serde_json::from_value(json).unwrap();
        assert!(d.target.is_none());
        assert!(d.referenced_resource.is_none());
        assert!(d.unresolved_endpoint.is_none());
        assert!(d.scope.is_none());
        assert!(d.first_observed_at.is_none());
        assert!(d.last_observed_at.is_none());
    }

    #[test]
    fn diagnostic_ignores_unknown_fields_from_newer_producers() {
        let json = serde_json::json!({
            "code": "INCOMPLETE_SCOPE",
            "severity": "WARNING",
            "message": "pods scope failed",
            "supporting_evidence_ids": [],
            "relevance": "WORKSPACE",
            "completeness_impact": "PARTIAL",
            "added_in_a_later_version": {"anything": true},
        });
        let d: Diagnostic = serde_json::from_value(json).unwrap();
        assert_eq!(d.code, "INCOMPLETE_SCOPE");
    }

    #[test]
    fn diagnostic_missing_a_required_field_is_rejected() {
        let json = serde_json::json!({
            "code": "INCOMPLETE_SCOPE",
            "severity": "WARNING",
            "message": "pods scope failed",
            "supporting_evidence_ids": [],
            "relevance": "WORKSPACE",
        });
        let err = serde_json::from_value::<Diagnostic>(json).unwrap_err();
        assert!(err.to_string().contains("completeness_impact"), "{err}");
    }

    // -----------------------------------------------------------------------
    // Diagnostic constructors
    // -----------------------------------------------------------------------

    #[test]
    fn unobserved_target_sets_code_severity_relevance_and_impact() {
        let target = k8s_service("payments/api");
        let d = Diagnostic::unobserved_target(target.clone());

        assert_eq!(d.code, diagnostic_code::UNOBSERVED_TARGET);
        assert_eq!(d.severity, DiagnosticSeverity::Error);
        assert_eq!(d.relevance, DiagnosticRelevance::Target);
        assert_eq!(d.completeness_impact, CompletenessStatus::Degraded);
        assert_eq!(d.target, Some(target));
        assert!(d.referenced_resource.is_none());
        assert!(d.supporting_evidence_ids.is_empty());
        assert!(d.message.contains("payments/api"), "{}", d.message);
        assert!(d.message.contains("kubernetes"), "{}", d.message);
    }

    #[test]
    fn new_keeps_every_field() {
        let evidence = vec![EvidenceId::new(), EvidenceId::new()];
        let d = Diagnostic::new(
            diagnostic_code::UNRESOLVED_REFERENCE,
            DiagnosticSeverity::Error,
            "service points at a missing deployment",
            Some(k8s_service("payments/api")),
            Some(k8s_service("payments/worker")),
            Some("10.0.0.7:9090".to_string()),
            Some("services".to_string()),
            evidence.clone(),
            DiagnosticRelevance::BlastRadius,
            CompletenessStatus::Degraded,
            Some("2026-10-01T10:00:00Z".to_string()),
            Some("2026-10-02T10:00:00Z".to_string()),
        );

        assert_eq!(d.code, "UNRESOLVED_REFERENCE");
        assert_eq!(d.severity, DiagnosticSeverity::Error);
        assert_eq!(d.message, "service points at a missing deployment");
        assert_eq!(d.target, Some(k8s_service("payments/api")));
        assert_eq!(d.referenced_resource, Some(k8s_service("payments/worker")));
        assert_eq!(d.unresolved_endpoint.as_deref(), Some("10.0.0.7:9090"));
        assert_eq!(d.scope.as_deref(), Some("services"));
        assert_eq!(d.supporting_evidence_ids, evidence);
        assert_eq!(d.relevance, DiagnosticRelevance::BlastRadius);
        assert_eq!(d.completeness_impact, CompletenessStatus::Degraded);
        assert_eq!(d.first_observed_at.as_deref(), Some("2026-10-01T10:00:00Z"));
        assert_eq!(d.last_observed_at.as_deref(), Some("2026-10-02T10:00:00Z"));
    }

    // -----------------------------------------------------------------------
    // Deterministic ordering
    // -----------------------------------------------------------------------

    #[test]
    fn sort_key_uses_empty_strings_for_missing_resources() {
        let d = diag("INCOMPLETE_SCOPE", "pods failed", None, None);
        assert_eq!(d.sort_key(), ("INCOMPLETE_SCOPE", "pods failed", "", ""));

        let d = diag(
            "UNRESOLVED_REFERENCE",
            "missing",
            Some("a/api"),
            Some("a/db"),
        );
        assert_eq!(
            d.sort_key(),
            ("UNRESOLVED_REFERENCE", "missing", "a/api", "a/db")
        );
    }

    #[test]
    fn sorting_by_sort_key_is_deterministic_regardless_of_input_order() {
        let expected = vec![
            diag("INCOMPLETE_SCOPE", "pods failed", None, None),
            diag("UNRESOLVED_REFERENCE", "missing", None, Some("a/db")),
            diag(
                "UNRESOLVED_REFERENCE",
                "missing",
                Some("a/api"),
                Some("a/cache"),
            ),
            diag(
                "UNRESOLVED_REFERENCE",
                "missing",
                Some("a/api"),
                Some("a/db"),
            ),
            diag("UNRESOLVED_REFERENCE", "other", Some("a/api"), None),
        ];
        // Vary the fields sort_key ignores so the order can't depend on them.
        let mut expected = expected;
        expected[0].severity = DiagnosticSeverity::Error;
        expected[1].relevance = DiagnosticRelevance::Target;
        expected[2].completeness_impact = CompletenessStatus::Degraded;
        expected[4].severity = DiagnosticSeverity::Info;

        let mut reversed = expected.clone();
        reversed.reverse();
        let mut rotated = expected.clone();
        rotated.rotate_left(2);

        for mut input in [reversed, rotated] {
            input.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
            assert_eq!(input, expected);
        }
    }

    #[test]
    fn sort_key_ignores_everything_but_code_message_and_resources() {
        // Locks in the current contract: sort_key is a partial key.
        let base = diag(
            "UNRESOLVED_REFERENCE",
            "missing",
            Some("a/api"),
            Some("a/db"),
        );
        let mut other = base.clone();
        other.severity = DiagnosticSeverity::Error;
        other.relevance = DiagnosticRelevance::Workspace;
        other.completeness_impact = CompletenessStatus::Degraded;
        other.unresolved_endpoint = Some("10.0.0.1:8080".to_string());
        other.scope = Some("pods".to_string());
        other.supporting_evidence_ids = vec![EvidenceId::new()];

        assert_ne!(base, other);
        assert_eq!(base.sort_key(), other.sort_key());

        // Ties keep their input order under the stable sort_by.
        let mut forward = vec![base.clone(), other.clone()];
        forward.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        assert_eq!(forward, vec![base.clone(), other.clone()]);

        let mut backward = vec![other.clone(), base.clone()];
        backward.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        assert_eq!(backward, vec![other, base]);
    }
}
