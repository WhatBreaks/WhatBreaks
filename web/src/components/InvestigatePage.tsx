import React, {
  useState,
  useEffect,
  useRef,
  useCallback,
  useMemo,
} from "react";
import {
  Search,
  ChevronDown,
  Trash2,
  Edit3,
  BarChart2,
  RefreshCw,
  Play,
  AlertTriangle,
  GitBranch,
  Database,
  AlertCircle,
  ChevronRight,
  ShieldAlert,
  CheckCircle,
  HelpCircle,
} from "lucide-react";
import type {
  ChangeType,
  ImpactAnswer,
  ImpactRequest,
  ResourceItem,
  ResourceIdentity,
  DependencyRelationship,
} from "../types";
import { analyzeImpact, listResources, listDependencies } from "../api/client";
import { DependencyGraph } from "./DependencyGraph";
import { parseProviderID, resourceKey, sameResourceIdentity } from "../utils/resource";

// ── Tab definitions ─────────────────────────────────────────────────────────
type TabId = "impacted" | "paths" | "evidence" | "assumptions" | "limitations" | "diagnostics";

interface Tab {
  id: TabId;
  label: string;
  count?: number;
}

// ── Impacted Resources Table ─────────────────────────────────────────────────
const ImpactedTable: React.FC<{
  answer: ImpactAnswer;
  onSelectResource?: (res: ResourceIdentity) => void;
}> = ({ answer, onSelectResource }) => {
  if (answer.impacted_resources.length === 0) {
    return (
      <div className="empty-state">
        <CheckCircle size={28} style={{ color: "var(--status-ok)" }} />
        <div className="empty-state-title">Zero Impact Detected</div>
        <div className="empty-state-sub">
          This change has no detectable downstream dependents based on currently discovered relationships.
          Zero impact does not guarantee zero operational risk if unobserved external clients exist.
        </div>
      </div>
    );
  }

  return (
    <div style={{ overflowX: "auto" }}>
      <table className="impact-table">
        <thead>
          <tr>
            <th>Impacted Resource</th>
            <th>Type</th>
            <th>Namespace</th>
            <th>Cluster</th>
            <th>Impact Nature</th>
            <th>Hops</th>
            <th style={{ width: 24 }}></th>
          </tr>
        </thead>
        <tbody>
          {answer.impacted_resources.map((ir, i) => {
            const parsed = parseProviderID(ir.resource.provider_id);
            const reason =
              ir.impact_reason ||
              (ir.impact_type
                ? ir.impact_type.replace(/_/g, " ")
                : `Depth ${ir.depth} dependency`);
            const isDirect = ir.depth === 1;

            return (
              <tr
                key={i}
                onClick={() => onSelectResource && onSelectResource(ir.resource)}
                style={{ cursor: onSelectResource ? "pointer" : "default" }}
              >
                <td>
                  <div className="cell-resource">
                    <Database size={12} style={{ color: "var(--text-dim)", flexShrink: 0 }} />
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 500 }}>
                      {parsed.name}
                    </span>
                  </div>
                </td>
                <td className="type-label">{ir.resource.resource_type}</td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-secondary)" }}>
                  {parsed.namespace || "—"}
                </td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-dim)" }}>
                  {parsed.cluster || "—"}
                </td>
                <td style={{ fontSize: 12, color: "var(--text-secondary)" }}>{reason}</td>
                <td>
                  <span className={`risk-badge ${isDirect ? "high" : "medium"}`}>
                    {isDirect ? "DIRECT" : `${ir.depth} HOPS`}
                  </span>
                </td>
                <td>
                  <ChevronRight size={12} style={{ color: "var(--text-dim)" }} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

// ── Dependency Paths Tab ─────────────────────────────────────────────────────
const PathsTab: React.FC<{ answer: ImpactAnswer }> = ({ answer }) => {
  if (!answer.paths || answer.paths.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">No dependency paths recorded</div>
        <div className="empty-state-sub">The analysis traversed no multi-hop paths.</div>
      </div>
    );
  }
  return (
    <div className="path-list">
      {answer.paths.map((p, i) => (
        <div key={i} className="path-row">
          <span style={{ color: "var(--text-dim)", minWidth: 20, fontSize: 10 }}>#{i + 1}</span>
          {p.resources.map((res, j) => {
            const parsed = parseProviderID(res.provider_id);
            return (
              <React.Fragment key={j}>
                <span className={`path-node${j === 0 ? " target-node" : ""}`} title={res.provider_id}>
                  {parsed.name}
                </span>
                {j < p.relationships.length && (
                  <>
                    <span style={{ color: "var(--text-dim)", fontSize: 10 }}>→</span>
                    <span className="path-edge">{p.relationships[j].kind}</span>
                    <span style={{ color: "var(--text-dim)", fontSize: 10 }}>→</span>
                  </>
                )}
              </React.Fragment>
            );
          })}
        </div>
      ))}
    </div>
  );
};

// ── Evidence Tab ─────────────────────────────────────────────────────────────
const EvidenceTab: React.FC<{ answer: ImpactAnswer }> = ({ answer }) => {
  if (!answer.evidence || answer.evidence.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">No evidence collected</div>
        <div className="empty-state-sub">No observation evidence was attached to this analysis.</div>
      </div>
    );
  }
  return (
    <div className="evidence-list">
      {answer.evidence.map((ev, i) => (
        <div key={i} className="evidence-item">
          <span className="evidence-id" title={ev.id}>{ev.id}</span>
          <span style={{ color: "var(--text-secondary)", fontSize: 11 }}>
            {ev.observation_type}
          </span>
          <span style={{ color: "var(--text-dim)", fontSize: 10 }}>
            {ev.source.provider}/{ev.source.collector}
          </span>
          <span style={{ color: "var(--text-dim)", fontSize: 10, marginLeft: "auto" }}>
            {new Date(ev.observed_at).toLocaleString()}
          </span>
        </div>
      ))}
    </div>
  );
};

// ── Assumptions / Limitations ─────────────────────────────────────────────────
const TextListTab: React.FC<{ items: string[]; emptyMsg: string }> = ({ items, emptyMsg }) => {
  if (items.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">{emptyMsg}</div>
      </div>
    );
  }
  return (
    <div className="text-list">
      {items.map((item, i) => (
        <div key={i} className="text-item">{item}</div>
      ))}
    </div>
  );
};

// ── Diagnostics Tab ──────────────────────────────────────────────────────────
const DiagnosticsTab: React.FC<{ diagnostics?: import("../types").DiagnosticItem[] }> = ({ diagnostics }) => {
  if (!diagnostics || diagnostics.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">No diagnostics emitted</div>
        <div className="empty-state-sub">Discovery and analysis completed with full confidence.</div>
      </div>
    );
  }
  return (
    <div className="evidence-list">
      {diagnostics.map((diag, i) => (
        <div
          key={i}
          className="evidence-item"
          style={{ flexDirection: "column", alignItems: "flex-start", gap: 4 }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, width: "100%" }}>
            <span
              className={`risk-badge ${
                diag.severity === "ERROR" ? "high" : diag.severity === "WARNING" ? "medium" : "low"
              }`}
              style={{ fontSize: 9 }}
            >
              {diag.severity}
            </span>
            <span style={{ fontFamily: "var(--font-mono)", fontSize: 11, fontWeight: 600 }}>
              {diag.code}
            </span>
            {diag.relevance && (
              <span style={{ fontSize: 10, color: "var(--text-dim)", marginLeft: "auto" }}>
                Relevance: {diag.relevance}
              </span>
            )}
          </div>
          <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>{diag.message}</div>
          {diag.completeness_impact && (
            <div style={{ fontSize: 10, color: "var(--text-dim)", fontStyle: "italic" }}>
              Completeness Impact: {diag.completeness_impact}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

// ── Topological Relationships Table (Exploration Mode) ──────────────────────
const RelationshipsTable: React.FC<{
  title: string;
  relationships: DependencyRelationship[];
  isIncoming: boolean;
  onSelectResource: (res: ResourceIdentity) => void;
}> = ({ title, relationships, isIncoming, onSelectResource }) => {
  if (relationships.length === 0) {
    return (
      <div className="empty-state" style={{ padding: "24px 0" }}>
        <div className="empty-state-title">No {title.toLowerCase()} discovered</div>
        <div className="empty-state-sub">
          {isIncoming
            ? "No other resources are observed depending directly on this resource."
            : "This resource does not observe any direct outgoing dependencies."}
        </div>
      </div>
    );
  }

  return (
    <div style={{ overflowX: "auto" }}>
      <table className="impact-table">
        <thead>
          <tr>
            <th>Connected Resource</th>
            <th>Type</th>
            <th>Namespace</th>
            <th>Cluster</th>
            <th>Relationship Kind</th>
            <th style={{ width: 24 }}></th>
          </tr>
        </thead>
        <tbody>
          {relationships.map((rel, i) => {
            const peer = isIncoming ? rel.source : rel.target;
            const parsed = parseProviderID(peer.provider_id);
            return (
              <tr
                key={i}
                onClick={() => onSelectResource(peer)}
                style={{ cursor: "pointer" }}
                title={`Explore ${peer.provider_id}`}
              >
                <td>
                  <div className="cell-resource">
                    <Database size={12} style={{ color: "var(--text-dim)", flexShrink: 0 }} />
                    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12, fontWeight: 500 }}>
                      {parsed.name}
                    </span>
                  </div>
                </td>
                <td className="type-label">{peer.resource_type}</td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-secondary)" }}>
                  {parsed.namespace || "—"}
                </td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-dim)" }}>
                  {parsed.cluster || "—"}
                </td>
                <td>
                  <span
                    className="risk-badge low"
                    style={{
                      background: "rgba(255,255,255,0.06)",
                      color: "var(--text-primary)",
                      fontFamily: "var(--font-mono)",
                      fontSize: 10,
                    }}
                  >
                    {isIncoming ? `← ${rel.kind}` : `${rel.kind} →`}
                  </span>
                </td>
                <td>
                  <ChevronRight size={12} style={{ color: "var(--text-dim)" }} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

// ── Main InvestigatePage ──────────────────────────────────────────────────────
export interface InvestigatePageProps {
  workspaceId: string;
  clusterId: string;
  initialResource?: ResourceItem | null;
}

export const InvestigatePage: React.FC<InvestigatePageProps> = ({
  workspaceId,
  clusterId,
  initialResource,
}) => {
  // Resources & Dependencies
  const [resources, setResources] = useState<ResourceItem[]>([]);
  const [dependencies, setDependencies] = useState<DependencyRelationship[]>([]);
  const [loadingResources, setLoadingResources] = useState(false);
  const [resourceError, setResourceError] = useState<string | null>(null);

  // Resource selector state
  const [searchText, setSearchText] = useState("");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [selectedResource, setSelectedResource] = useState<ResourceItem | null>(initialResource ?? null);
  const searchRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Change form
  const [changeType, setChangeType] = useState<ChangeType>("DELETE");
  const [details, setDetails] = useState("");

  // Analysis
  const [analyzing, setAnalyzing] = useState(false);
  const [answer, setAnswer] = useState<ImpactAnswer | null>(null);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [analysisTime, setAnalysisTime] = useState<Date | null>(null);

  // Tabs
  const [activeTab, setActiveTab] = useState<TabId>("impacted");
  const [explorationTab, setExplorationTab] = useState<"incoming" | "outgoing">("incoming");

  // Immediate neighborhood computation for exploration
  const targetKey = selectedResource ? resourceKey(selectedResource) : "";
  const incomingDependencies = useMemo(() => {
    if (!targetKey) return [];
    return dependencies.filter((d) => resourceKey(d.target) === targetKey);
  }, [dependencies, targetKey]);

  const outgoingDependencies = useMemo(() => {
    if (!targetKey) return [];
    return dependencies.filter((d) => resourceKey(d.source) === targetKey);
  }, [dependencies, targetKey]);

  // Sync initialResource if prop updates
  useEffect(() => {
    if (initialResource) {
      setSelectedResource(initialResource);
      setSearchText(initialResource.provider_id);
      setAnswer(null);
      setAnalysisError(null);
    }
  }, [initialResource]);

  // Load resources and dependencies whenever workspaceId or clusterId changes
  useEffect(() => {
    let active = true;
    const load = async () => {
      setLoadingResources(true);
      setResourceError(null);
      try {
        const [res, deps] = await Promise.all([
          listResources(workspaceId, {
            cluster: clusterId || undefined,
          }),
          listDependencies(workspaceId, clusterId || undefined),
        ]);
        if (active) {
          setResources(res.resources);
          setDependencies(deps.relationships || []);
        }
      } catch (e) {
        if (active) {
          const msg = e instanceof Error ? e.message : "Failed to load resources";
          setResourceError(msg);
          setResources([]);
          setDependencies([]);
        }
      } finally {
        if (active) setLoadingResources(false);
      }
    };
    load();
    return () => {
      active = false;
    };
  }, [workspaceId, clusterId]);

  // Validate selected resource against cluster boundary whenever clusterId changes
  useEffect(() => {
    if (!selectedResource) return;
    const parsed = parseProviderID(selectedResource.provider_id);
    if (clusterId && clusterId !== "all" && parsed.cluster && parsed.cluster !== clusterId) {
      // Invalidate stale selection from a different cluster
      setSelectedResource(null);
      setSearchText("");
      setAnswer(null);
      setAnalysisError(null);
    }
  }, [clusterId, selectedResource]);

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node) &&
        searchRef.current &&
        !searchRef.current.contains(e.target as Node)
      ) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  // Filtered resources for the search box
  const filteredResources = useMemo(() => {
    return resources.filter((r) => {
      // Cluster boundary check
      if (clusterId && clusterId !== "all") {
        const parsed = parseProviderID(r.provider_id);
        if (parsed.cluster && parsed.cluster !== clusterId) {
          return false;
        }
      }
      if (!searchText.trim()) return true;
      const q = searchText.toLowerCase();
      return (
        r.provider_id.toLowerCase().includes(q) ||
        r.resource_type.toLowerCase().includes(q) ||
        r.provider.toLowerCase().includes(q)
      );
    });
  }, [resources, clusterId, searchText]);

  const handleSelectChangeType = (ct: ChangeType) => {
    setChangeType(ct);
    if (answer && answer.change_assessment?.change_type && answer.change_assessment.change_type !== ct) {
      setAnswer(null); // Invalidate results if user switches to a different proposed change type
    }
  };

  const handleSelectResource = useCallback((res: ResourceItem) => {
    setSelectedResource(res);
    setSearchText(res.provider_id);
    setDropdownOpen(false);
    setAnswer(null);
    setAnalysisError(null);
  }, []);

  const handleAnalyze = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedResource) return;
    setAnalyzing(true);
    setAnalysisError(null);
    try {
      const req: ImpactRequest = {
        target: {
          provider: selectedResource.provider,
          resource_type: selectedResource.resource_type,
          provider_id: selectedResource.provider_id,
        },
        direction: "incoming",
        max_depth: 3,
        workspace_id: workspaceId,
        proposed_change: {
          change_type: changeType,
          details: details.trim() || undefined,
        },
      };
      const res = await analyzeImpact(req);
      setAnswer(res);
      setAnalysisTime(new Date());
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Analysis failed";
      setAnalysisError(msg);
      setAnswer(null);
    } finally {
      setAnalyzing(false);
    }
  };

  const parsedTarget = selectedResource
    ? parseProviderID(selectedResource.provider_id)
    : null;

  // Tabs for results
  const tabs: Tab[] = answer
    ? [
        { id: "impacted", label: "Impacted Resources", count: answer.impacted_resources.length },
        { id: "paths", label: "Dependency Paths", count: answer.paths?.length ?? 0 },
        { id: "evidence", label: "Evidence", count: answer.evidence?.length ?? 0 },
        { id: "assumptions", label: "Assumptions", count: answer.change_assessment?.assumptions.length ?? 0 },
        { id: "limitations", label: "Limitations", count: answer.change_assessment?.limitations.length ?? 0 },
        ...(answer.diagnostics && answer.diagnostics.length > 0
          ? [{ id: "diagnostics" as TabId, label: "Diagnostics", count: answer.diagnostics.length }]
          : []),
      ]
    : [];

  // Timer for relative time
  const [currentTimestamp, setCurrentTimestamp] = useState(() => Date.now());
  useEffect(() => {
    if (!analysisTime) return;
    const id = setInterval(() => setCurrentTimestamp(Date.now()), 1000);
    return () => clearInterval(id);
  }, [analysisTime]);

  const timeAgo = (d: Date): string => {
    const secs = Math.max(0, Math.floor((currentTimestamp - d.getTime()) / 1000));
    if (secs < 60) return `${secs}s ago`;
    const mins = Math.floor(secs / 60);
    return `${mins}m ${secs % 60}s ago`;
  };

  return (
    <div className="investigate-page">
      {/* ── Heading ── */}
      <div className="page-section-label">Impact Analysis</div>
      <h1 className="page-heading">What breaks if I change this resource?</h1>
      <p className="page-subtitle">
        Select a resource, propose an operational change, and evaluate deterministic propagation across your cluster.
      </p>

      {/* ── Analysis form ── */}
      <form className="analysis-form" onSubmit={handleAnalyze}>
        {/* 1. Target resource */}
        <div className="form-section" style={{ gridColumn: 1 }}>
          <div className="form-section-label">
            1. Target Resource {clusterId && clusterId !== "all" && `[${clusterId}]`}
          </div>
          <div className="resource-search-wrap" style={{ position: "relative" }}>
            <Search size={13} className="resource-search-icon" />
            <input
              ref={searchRef}
              type="text"
              className="resource-search-input"
              placeholder={
                clusterId && clusterId !== "all"
                  ? `Search ${clusterId} resources...`
                  : "Search resources (deployment, service, pod...)"
              }
              value={searchText}
              onChange={(e) => {
                setSearchText(e.target.value);
                setDropdownOpen(true);
                if (!e.target.value) setSelectedResource(null);
              }}
              onFocus={() => setDropdownOpen(true)}
              aria-label="Search and select a target resource"
              aria-expanded={dropdownOpen}
              aria-haspopup="listbox"
              autoComplete="off"
            />
            <ChevronDown size={13} className="resource-search-chevron" />

            {/* Dropdown */}
            {dropdownOpen && (
              <div
                ref={dropdownRef}
                className="resource-dropdown"
                role="listbox"
                aria-label="Resource search results"
              >
                {loadingResources ? (
                  <div className="resource-dropdown-empty">
                    <span className="loading-dots">
                      <span className="loading-dot" />
                      <span className="loading-dot" />
                      <span className="loading-dot" />
                    </span>
                  </div>
                ) : resourceError ? (
                  <div className="resource-dropdown-empty" style={{ color: "var(--status-error)" }}>
                    {resourceError}
                  </div>
                ) : filteredResources.length === 0 ? (
                  <div className="resource-dropdown-empty">
                    No resources match {clusterId ? `in cluster ${clusterId}` : ""}
                  </div>
                ) : (
                  filteredResources.slice(0, 60).map((res) => {
                    const parsed = parseProviderID(res.provider_id);
                    const isSelected = sameResourceIdentity(selectedResource, res);
                    return (
                      <div
                        key={`${res.provider}/${res.resource_type}/${res.provider_id}`}
                        className={`resource-dropdown-item${isSelected ? " highlighted" : ""}`}
                        role="option"
                        aria-selected={isSelected}
                        onClick={() => handleSelectResource(res)}
                      >
                        <div className="resource-dropdown-item-id">
                          <span style={{ color: "var(--text-dim)", marginRight: 4 }}>
                            {parsed.cluster ? `${parsed.cluster}/` : ""}
                            {parsed.namespace ? `${parsed.namespace}/` : ""}
                          </span>
                          <strong style={{ color: "var(--text-primary)" }}>{parsed.name}</strong>
                        </div>
                        <div className="resource-dropdown-item-meta">
                          {res.resource_type} · {res.provider}
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            )}
          </div>
        </div>

        {/* 2. Proposed change */}
        <div className="form-section" style={{ gridColumn: 2 }}>
          <div className="form-section-label">2. Proposed Change</div>
          <div className="change-type-grid">
            {(["DELETE", "UPDATE", "SCALE", "REPLACE"] as ChangeType[]).map((ct) => {
              const Icon =
                ct === "DELETE" ? Trash2 :
                ct === "UPDATE" ? Edit3 :
                ct === "SCALE"  ? BarChart2 :
                RefreshCw;
              return (
                <button
                  key={ct}
                  type="button"
                  className={`change-btn${changeType === ct ? " selected" : ""}`}
                  onClick={() => handleSelectChangeType(ct)}
                  aria-pressed={changeType === ct}
                >
                  <Icon size={13} />
                  {ct}
                </button>
              );
            })}
          </div>
        </div>

        {/* 3. Change details */}
        <div className="form-section details-section" style={{ gridColumn: 3 }}>
          <div className="form-section-label">3. Change Details (Optional)</div>
          <input
            type="text"
            className="details-input"
            placeholder="Add details (e.g. replicas: 0, image tag...)"
            value={details}
            onChange={(e) => setDetails(e.target.value)}
            aria-label="Optional change details"
          />
        </div>

        {/* Analyze button */}
        <div className="analyze-section" style={{ gridColumn: 4 }}>
          <button
            type="submit"
            className="analyze-btn"
            disabled={!selectedResource || analyzing}
            aria-busy={analyzing}
          >
            <Play size={13} />
            {analyzing ? "Analyzing…" : "Analyze Impact"}
          </button>
        </div>
      </form>

      {/* ── Analysis error ── */}
      {analysisError && (
        <div className="error-state" style={{ marginBottom: 16 }}>
          <AlertTriangle size={18} style={{ flexShrink: 0, marginTop: 2 }} />
          <div>
            <strong style={{ display: "block", marginBottom: 4 }}>Analysis Request Failed</strong>
            <p style={{ margin: "0 0 6px 0" }}>{analysisError}</p>
            <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
              Guidance: Verify that the Core Engine gRPC service is healthy and discovery state is synchronized for workspace <code>{workspaceId}</code>.
            </div>
          </div>
        </div>
      )}

      {/* ── Loading state ── */}
      {analyzing && (
        <div className="empty-state" style={{ padding: "40px 0" }}>
          <span className="loading-dots">
            <span className="loading-dot" />
            <span className="loading-dot" />
            <span className="loading-dot" />
          </span>
          <div className="empty-state-title">Traversing dependency graph…</div>
          <div className="empty-state-sub">
            Evaluating multi-hop propagation paths and synthesizing factual evidence.
          </div>
        </div>
      )}

      {/* ── Initial empty state (when no resource selected) ── */}
      {!analyzing && !selectedResource && !analysisError && (
        <div className="empty-state" style={{ padding: "40px 0", opacity: 0.7 }}>
          <AlertCircle size={28} style={{ color: "var(--text-dim)" }} />
          <div className="empty-state-title">Select a resource to explore relationships and analyze impact</div>
          <div className="empty-state-sub">
            {clusterId && clusterId !== "all"
              ? `Currently scoped to cluster ${clusterId}. Select an infrastructure component above.`
              : "Select a discovered infrastructure resource above to inspect its topology or evaluate potential blast radius."}
          </div>
        </div>
      )}

      {/* ── Resource Exploration Mode (Resource selected, no impact simulation run yet) ── */}
      {!analyzing && selectedResource && !answer && (
        <div className="results-section">
          {/* Header */}
          <div style={{ marginBottom: 14 }}>
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between" }}>
              <div>
                <div className="page-section-label" style={{ marginBottom: 2 }}>Resource Exploration</div>
                <div className="results-resource-name">
                  {parsedTarget?.name || selectedResource.provider_id}
                </div>
                <div className="results-resource-meta">
                  <span>{selectedResource.resource_type}</span>
                  {parsedTarget?.cluster && (
                    <>
                      <span className="results-meta-sep">|</span>
                      <span>cluster: {parsedTarget.cluster}</span>
                    </>
                  )}
                  {parsedTarget?.namespace && (
                    <>
                      <span className="results-meta-sep">|</span>
                      <span>namespace: {parsedTarget.namespace}</span>
                    </>
                  )}
                  <span className="results-meta-sep">|</span>
                  <span style={{ color: "var(--text-dim)" }}>{selectedResource.provider_id}</span>
                </div>
              </div>
              <div className="results-status">
                <span
                  className="risk-badge low"
                  style={{
                    background: "rgba(255,255,255,0.08)",
                    color: "var(--text-primary)",
                    border: "1px solid var(--border)",
                    fontSize: 10,
                  }}
                >
                  EXPLORATION MODE
                </span>
              </div>
            </div>
          </div>

          {/* Exploration guidance banner */}
          <div
            style={{
              background: "rgba(255, 255, 255, 0.03)",
              border: "1px solid var(--border)",
              color: "var(--text-secondary)",
              padding: "10px 14px",
              marginBottom: 16,
              fontSize: 12,
              display: "flex",
              gap: 10,
              alignItems: "center",
            }}
          >
            <HelpCircle size={16} style={{ flexShrink: 0, color: "var(--text-dim)" }} />
            <div>
              <strong>Topology Exploration:</strong> Showing immediate incoming and outgoing relationships for this resource.
              Connected resources are neutral dependencies and not marked as impacted. To simulate potential blast radius for an operational change, choose a change type above and click <strong>Analyze Impact</strong>.
            </div>
          </div>

          {/* Summary Stats for Exploration */}
          <div className="summary-grid">
            <div className="summary-stat">
              <GitBranch size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">{incomingDependencies.length}</div>
                <div className="summary-stat-label">Inbound Dependents</div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  Resources calling or depending on target
                </div>
              </div>
            </div>
            <div className="summary-stat">
              <Database size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">{outgoingDependencies.length}</div>
                <div className="summary-stat-label">Outbound Dependencies</div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  Direct services and configs consumed
                </div>
              </div>
            </div>
            <div className="summary-stat">
              <Database size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">
                  {incomingDependencies.length + outgoingDependencies.length}
                </div>
                <div className="summary-stat-label">Total Connected Edges</div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  Discovered topological linkages
                </div>
              </div>
            </div>
            <div className="summary-stat">
              <HelpCircle size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">Neutral</div>
                <div className="summary-stat-label">Impact Status</div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  Click Analyze Impact to simulate
                </div>
              </div>
            </div>
          </div>

          {/* Dependency Graph in Exploration Mode */}
          <DependencyGraph
            target={selectedResource}
            answer={null}
            allRelationships={dependencies}
            onSelectNode={(nodeId) => {
              const found = resources.find((r) => sameResourceIdentity(r, nodeId));
              handleSelectResource(
                found || {
                  provider: nodeId.provider,
                  resource_type: nodeId.resource_type,
                  provider_id: nodeId.provider_id,
                  first_observed_at: "",
                  last_observed_at: "",
                }
              );
            }}
          />

          {/* Exploration Relationships Tabs */}
          <div className="results-tabs">
            <div className="tab-bar" role="tablist" aria-label="Topological relationships">
              <button
                className={`tab-btn${explorationTab === "incoming" ? " active" : ""}`}
                role="tab"
                aria-selected={explorationTab === "incoming"}
                onClick={() => setExplorationTab("incoming")}
              >
                Inbound Dependents ({incomingDependencies.length})
              </button>
              <button
                className={`tab-btn${explorationTab === "outgoing" ? " active" : ""}`}
                role="tab"
                aria-selected={explorationTab === "outgoing"}
                onClick={() => setExplorationTab("outgoing")}
              >
                Outbound Dependencies ({outgoingDependencies.length})
              </button>
            </div>

            <div className="tab-panel" role="tabpanel">
              {explorationTab === "incoming" ? (
                <RelationshipsTable
                  title="Inbound Dependents"
                  relationships={incomingDependencies}
                  isIncoming={true}
                  onSelectResource={(peer) => {
                    const found = resources.find((r) => sameResourceIdentity(r, peer));
                    handleSelectResource(
                      found || {
                        provider: peer.provider,
                        resource_type: peer.resource_type,
                        provider_id: peer.provider_id,
                        first_observed_at: "",
                        last_observed_at: "",
                      }
                    );
                  }}
                />
              ) : (
                <RelationshipsTable
                  title="Outbound Dependencies"
                  relationships={outgoingDependencies}
                  isIncoming={false}
                  onSelectResource={(peer) => {
                    const found = resources.find((r) => sameResourceIdentity(r, peer));
                    handleSelectResource(
                      found || {
                        provider: peer.provider,
                        resource_type: peer.resource_type,
                        provider_id: peer.provider_id,
                        first_observed_at: "",
                        last_observed_at: "",
                      }
                    );
                  }}
                />
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── Results ── */}
      {!analyzing && answer && (
        <div className="results-section">
          {/* Header */}
          <div style={{ marginBottom: 14 }}>
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between" }}>
              <div>
                <div className="page-section-label" style={{ marginBottom: 2 }}>Analysis Results</div>
                <div className="results-resource-name">
                  {parsedTarget?.name || answer.target.provider_id}
                </div>
                <div className="results-resource-meta">
                  <span>{answer.target.resource_type}</span>
                  {parsedTarget?.cluster && (
                    <>
                      <span className="results-meta-sep">|</span>
                      <span>cluster: {parsedTarget.cluster}</span>
                    </>
                  )}
                  {parsedTarget?.namespace && (
                    <>
                      <span className="results-meta-sep">|</span>
                      <span>namespace: {parsedTarget.namespace}</span>
                    </>
                  )}
                  <span className="results-meta-sep">|</span>
                  <span style={{ color: "var(--text-dim)" }}>{answer.target.provider_id}</span>
                </div>
              </div>
              {analysisTime && (
                <div className="results-status">
                  <div className="results-status-dot" />
                  <span className="results-status-label">Analysis Complete</span>
                  <span className="results-status-time">{timeAgo(analysisTime)}</span>
                </div>
              )}
            </div>
          </div>

          {/* Completeness / Partial notification if applicable */}
          {answer.completeness && !answer.completeness.is_complete && (
            <div
              style={{
                background: "rgba(245, 158, 11, 0.1)",
                border: "1px solid var(--status-warn)",
                color: "#fde68a",
                padding: "10px 14px",
                marginBottom: 16,
                fontSize: 12,
                display: "flex",
                gap: 10,
                alignItems: "center",
              }}
            >
              <ShieldAlert size={16} style={{ flexShrink: 0 }} />
              <div>
                <strong>Analysis Completeness: {answer.completeness.status}</strong>
                {answer.completeness.reasons && answer.completeness.reasons.length > 0 && (
                  <span style={{ marginLeft: 6 }}>
                    ({answer.completeness.reasons.join(", ")})
                  </span>
                )}
                <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>
                  Some graph edges may be omitted because discovery collection was partial or degraded.
                </div>
              </div>
            </div>
          )}

          {/* Trustworthy Summary Stats (Derived strictly from real API response) */}
          <div className="summary-grid">
            <div className="summary-stat">
              <GitBranch size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">{answer.summary.impacted_count}</div>
                <div className="summary-stat-label">Impacted resources</div>
                <div style={{ fontSize: 10, color: "var(--text-dim)", marginTop: 2 }}>
                  {answer.summary.direct_count} direct · {answer.summary.indirect_count} indirect
                </div>
              </div>
            </div>
            <div className="summary-stat">
              <Database size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">
                  {answer.summary.max_depth} {answer.summary.max_depth === 1 ? "hop" : "hops"}
                </div>
                <div className="summary-stat-label">Max traversal depth</div>
              </div>
            </div>
            <div className="summary-stat">
              <Database size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">{answer.paths?.length ?? 0}</div>
                <div className="summary-stat-label">Propagation paths</div>
              </div>
            </div>
            <div className="summary-stat">
              <HelpCircle size={20} className="summary-stat-icon" />
              <div className="summary-stat-body">
                <div className="summary-stat-value">{answer.evidence?.length ?? 0}</div>
                <div className="summary-stat-label">Evidence items</div>
              </div>
            </div>
          </div>

          {/* Dependency Graph */}
          <DependencyGraph
            target={selectedResource || answer.target}
            answer={answer}
            allRelationships={dependencies}
            onSelectNode={(nodeId) => {
              const found = resources.find((r) => sameResourceIdentity(r, nodeId));
              handleSelectResource(
                found || {
                  provider: nodeId.provider,
                  resource_type: nodeId.resource_type,
                  provider_id: nodeId.provider_id,
                  first_observed_at: "",
                  last_observed_at: "",
                }
              );
            }}
          />

          {/* Tabbed Results */}
          <div className="results-tabs">
            <div className="tab-bar" role="tablist" aria-label="Analysis results">
              {tabs.map((t) => (
                <button
                  key={t.id}
                  className={`tab-btn${activeTab === t.id ? " active" : ""}`}
                  role="tab"
                  aria-selected={activeTab === t.id}
                  aria-controls={`tabpanel-${t.id}`}
                  id={`tab-${t.id}`}
                  onClick={() => setActiveTab(t.id)}
                >
                  {t.label}
                  {t.count !== undefined && ` (${t.count})`}
                </button>
              ))}
            </div>

            <div
              className="tab-panel"
              role="tabpanel"
              id={`tabpanel-${activeTab}`}
              aria-labelledby={`tab-${activeTab}`}
            >
              {activeTab === "impacted" && (
                <ImpactedTable
                  answer={answer}
                  onSelectResource={(res) => {
                    const found = resources.find((r) => sameResourceIdentity(r, res));
                    handleSelectResource(
                      found || {
                        provider: res.provider,
                        resource_type: res.resource_type,
                        provider_id: res.provider_id,
                        first_observed_at: "",
                        last_observed_at: "",
                      }
                    );
                  }}
                />
              )}
              {activeTab === "paths" && <PathsTab answer={answer} />}
              {activeTab === "evidence" && <EvidenceTab answer={answer} />}
              {activeTab === "assumptions" && (
                <TextListTab
                  items={answer.change_assessment?.assumptions ?? []}
                  emptyMsg="No assumptions recorded for this analysis"
                />
              )}
              {activeTab === "limitations" && (
                <TextListTab
                  items={answer.change_assessment?.limitations ?? []}
                  emptyMsg="No limitations recorded for this analysis"
                />
              )}
              {activeTab === "diagnostics" && <DiagnosticsTab diagnostics={answer.diagnostics} />}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
