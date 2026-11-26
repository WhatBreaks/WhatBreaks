use std::num::NonZeroUsize;

use crate::{
    AnalysisCompleteness, AnswerEngine, AnswerRequest, Evidence, Graph, ImpactAnswer,
    ProvenanceStore, ResourceIdentity, VersionCompatibilityAssessment,
};

/// Trace incoming dependency paths up to `max_depth` for an upgrade with breaking claims.
///
/// Returns `None` without applicable breaking claims. The answer reports possible
/// dependents, their hop counts and paths, and any gaps in the available topology.
/// Whether a particular claim affects a dependent still depends on how it uses
/// the upgraded resource. Claim details and sources stay in `assessment`.
pub fn candidate_dependency_impact(
    assessment: &VersionCompatibilityAssessment<'_>,
    target: &ResourceIdentity,
    max_depth: NonZeroUsize,
    graph: &Graph,
    provenance: &ProvenanceStore,
    evidence: &[Evidence],
) -> Option<ImpactAnswer> {
    if assessment.breaking_claims.is_empty() {
        return None;
    }

    // Generic incoming traversal keeps indirect candidates. Configuration UPDATE
    // is deliberately limited to one hop and would omit upgrade candidates.
    let request = AnswerRequest::incoming(target.clone(), max_depth.get());
    let mut answer = AnswerEngine::analyze(graph, provenance, evidence, &request);
    if answer.impacted_resources.is_empty() && answer.completeness.is_complete {
        answer.completeness = AnalysisCompleteness::partial(vec![
            "No qualifying incoming dependencies were observed in this graph.".into(),
        ]);
    }
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyze_version_compatibility, ClaimEffect, DottedNumericOrdering, KnowledgeEvidence,
        KnowledgeEvidenceId, Provider, Relationship, RelationshipKind, ResourceKind,
        SourceAuthority, SourceRef, VersionRange, VersionRef, VersionTransition,
    };

    fn resource(name: &str) -> ResourceIdentity {
        ResourceIdentity::new(Provider::new("local"), ResourceKind::new("workload"), name)
    }

    fn claim(effect: ClaimEffect) -> KnowledgeEvidence {
        KnowledgeEvidence {
            id: KnowledgeEvidenceId::new("release-note"),
            component: "example".into(),
            applies_from: VersionRange::new("semver", None, Some("2.0".into()), true, false),
            applies_to: VersionRange::new("semver", Some("2.0".into()), None, true, false),
            effect,
            claim: "An older client setting is no longer accepted".into(),
            source: SourceRef {
                url: "https://example.test/notes".into(),
                title: "Release notes".into(),
                reviewed_on: "2026-11-25".into(),
                content_hash: "test-fixture".into(),
                authority: SourceAuthority::Official,
            },
        }
    }

    fn impact(graph: &Graph, effect: ClaimEffect, depth: usize) -> Option<ImpactAnswer> {
        let transition = VersionTransition::new(
            "example",
            VersionRef::known("semver", "1.0"),
            VersionRef::known("semver", "2.0"),
        );
        let records = [claim(effect)];
        let assessment =
            analyze_version_compatibility(&transition, &records, &DottedNumericOrdering);
        candidate_dependency_impact(
            &assessment,
            &resource("upgrade"),
            NonZeroUsize::new(depth).unwrap(),
            graph,
            &ProvenanceStore::new(),
            &[],
        )
    }

    #[test]
    fn reuses_directional_paths_and_hop_counts_in_a_cycle() {
        let mut graph = Graph::new();
        for (source, target, kind) in [
            ("api", "upgrade", RelationshipKind::CALLS),
            ("web", "api", RelationshipKind::DEPENDS_ON),
            ("upgrade", "web", RelationshipKind::CALLS),
            ("owner", "upgrade", RelationshipKind::OWNS),
        ] {
            graph.add_relationship(Relationship::new(resource(source), resource(target), kind));
        }

        let answer = impact(&graph, ClaimEffect::Breaking, 5).unwrap();
        assert_eq!(answer.summary.direct_count, 1);
        assert_eq!(answer.summary.indirect_count, 1);
        assert_eq!(answer.impacted_resources[0].resource, resource("api"));
        assert_eq!(answer.impacted_resources[1].resource, resource("web"));
        assert!(answer.paths.iter().any(|path| {
            path.resources == [resource("web"), resource("api"), resource("upgrade")]
        }));
        assert_eq!(
            impact(&graph, ClaimEffect::Breaking, 1)
                .unwrap()
                .summary
                .impacted_count,
            1
        );
    }

    #[test]
    fn preserves_missing_target_and_reports_absent_dependents() {
        let mut graph = Graph::new();
        let absent = impact(&graph, ClaimEffect::Breaking, 2).unwrap();
        assert!(absent.impacted_resources.is_empty());
        assert!(!absent.completeness.is_complete);
        assert!(absent
            .diagnostics
            .iter()
            .any(|diag| diag.code == "UNOBSERVED_TARGET"));

        graph.add_resource(resource("upgrade"));
        let isolated = impact(&graph, ClaimEffect::Breaking, 2).unwrap();
        assert!(isolated.impacted_resources.is_empty());
        assert!(isolated.completeness.reasons[0].contains("No qualifying incoming dependencies"));
    }

    #[test]
    fn non_breaking_claims_do_not_propagate_risk() {
        assert!(impact(&Graph::new(), ClaimEffect::NonBreaking, 1).is_none());
    }
}
