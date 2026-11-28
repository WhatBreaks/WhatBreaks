export interface ResourceIdentity {
  provider: string;
  resource_type: string;
  provider_id: string;
}

export interface ResourceItem {
  provider: string;
  resource_type: string;
  provider_id: string;
  first_observed_at: string;
  last_observed_at: string;
}

export interface ListResourcesResponse {
  workspace_id: string;
  total: number;
  resources: ResourceItem[];
  clusters?: string[];
}

export interface ResourceRelationshipItem {
  source: ResourceIdentity;
  target: ResourceIdentity;
  kind: string;
  category: string;
  direction: "OUTGOING" | "INCOMING";
  peer: ResourceIdentity;
  evidence_ids?: string[];
  first_observed_at: string;
  last_observed_at: string;
}

export interface DependencyRelationship {
  source: ResourceIdentity;
  target: ResourceIdentity;
  kind: string;
  category: string;
  first_observed_at: string;
  last_observed_at: string;
}

export interface ListDependenciesResponse {
  workspace_id: string;
  total: number;
  relationships: DependencyRelationship[];
}

export interface ComponentHealth {
  status: "healthy" | "degraded" | "unavailable" | "not_configured" | string;
  message?: string;
  details?: Record<string, any>;
}

export interface ClusterDetail {
  name: string;
  active: boolean;
  connected: boolean;
  status: "not_configured" | "unreachable" | "pending_sync" | "connected_empty" | "connected_with_resources" | string;
  resource_count: number;
  has_synced: boolean;
  last_synced_at?: string;
  error?: string;
}

export interface ListClustersResponse {
  workspace_id: string;
  active_cluster?: string;
  configured: boolean;
  connected: boolean;
  status: string;
  error?: string;
  clusters: ClusterDetail[];
}

export interface SystemHealthResponse {
  status: "healthy" | "degraded" | "unavailable" | string;
  timestamp: string;
  uptime: number;
  environment: string;
  components: Record<string, ComponentHealth>;
}

export interface ResourceDetailResponse {
  workspace_id: string;
  resource: ResourceItem;
  relationships: ResourceRelationshipItem[];
}

export interface DiscoverySyncResponse {
  workspace_id: string;
  status: "COMPLETED" | "PARTIAL";
  evidence_count: number;
  resources_created: number;
  resources_updated: number;
  resources_total: number;
  relationships_created: number;
  relationships_total: number;
  conflicts_count: number;
  duration_ms: number;
}

export type ChangeType = "DELETE" | "UPDATE" | "SCALE" | "REPLACE";

export interface ProposedChange {
  change_type: ChangeType;
  details?: string;
}

export interface ImpactRequest {
  target: ResourceIdentity;
  direction: "incoming" | "outgoing";
  max_depth: number;
  workspace_id?: string;
  proposed_change?: ProposedChange;
}

export interface ImpactSummary {
  impacted_count: number;
  direct_count: number;
  indirect_count: number;
  max_depth: number;
}

export interface ImpactedResource {
  resource: ResourceIdentity;
  depth: number;
  impact_type?: string;
  impact_reason?: string;
}

export interface AnswerRelationship {
  relationship: {
    source: ResourceIdentity;
    target: ResourceIdentity;
    kind: string;
    category: string;
  };
  state: string;
  evidence_ids: string[];
}

export interface ImpactPath {
  resources: ResourceIdentity[];
  relationships: {
    source: ResourceIdentity;
    target: ResourceIdentity;
    kind: string;
    category: string;
  }[];
}

export interface AnswerEvidence {
  id: string;
  source: {
    provider: string;
    collector: string;
  };
  observed_at: string;
  observation_type: string;
}

export interface ExplanationFact {
  path: ImpactPath;
  relationships: AnswerRelationship[];
  evidence_ids: string[];
}

export interface ChangeAssessment {
  change_type: ChangeType;
  impact_nature: string;
  assumptions?: string[];
  limitations?: string[];
}

export interface AnalysisCompleteness {
  is_complete: boolean;
  status: "COMPLETE" | "PARTIAL" | "DEGRADED" | string;
  reasons?: string[];
}

export interface DiagnosticItem {
  code: string;
  severity: "INFO" | "WARNING" | "ERROR" | string;
  message: string;
  relevance?: string;
  completeness_impact?: string;
  evidence_ids?: string[];
}

export interface ImpactAnswer {
  target: ResourceIdentity;
  summary: ImpactSummary;
  impacted_resources: ImpactedResource[];
  relationships: AnswerRelationship[];
  paths: ImpactPath[];
  evidence: AnswerEvidence[];
  explanation_facts: ExplanationFact[];
  change_assessment?: ChangeAssessment;
  completeness?: AnalysisCompleteness;
  diagnostics?: DiagnosticItem[];
}

export interface ApiError {
  error: string;
  code: string;
}

export type RecoveryStatus =
  | "not_started"
  | "diagnosing"
  | "fix_available"
  | "fixing"
  | "recovered"
  | "failed";

export interface DiagnosticStep {
  name: string;
  status: "ok" | "warn" | "failed" | "skipped";
  message: string;
  duration_ms: number;
  details?: Record<string, any>;
}

export interface DiagnosticAnalysis {
  probable_cause: string;
  confidence: "HIGH" | "MEDIUM" | "LOW";
  category: string;
  supporting_evidence: string[];
  recommended_action: string;
  fix_type: string;
  fix_available: boolean;
  mutating: boolean;
  potential_effects?: string;
}

export interface ComponentDiagnosticResult {
  component: string;
  status: string;
  timestamp: string;
  current_error?: string;
  steps: DiagnosticStep[];
  analysis: DiagnosticAnalysis;
}

export interface SystemDiagnoseResponse {
  workspace_id: string;
  timestamp: string;
  component?: string;
  results: Record<string, ComponentDiagnosticResult>;
}

export interface RecoveryRequest {
  component: string;
  action: string;
  confirmed: boolean;
}

export interface RecoveryResponse {
  workspace_id: string;
  component: string;
  action: string;
  status: "recovered" | "failed";
  verified_healthy: boolean;
  message: string;
  duration_ms: number;
  health?: ComponentHealth;
  next_steps?: string;
}
