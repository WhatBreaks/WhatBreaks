import React, {
  useRef,
  useState,
  useEffect,
  useCallback,
  useMemo,
} from "react";
import { Maximize2, Minus, Plus, Minimize2, Table, Network } from "lucide-react";
import type { ImpactAnswer, ResourceIdentity, DependencyRelationship } from "../types";
import {
  NODE_W,
  NODE_H,
  ARROW_SIZE,
  type ImpactVisualStatus,
  type GraphNode,
  type GraphEdge,
  buildGraph,
  edgePath,
  shouldZoomOnWheel,
  shouldActivateNode,
  updateMaxDragDistance,
  parseProviderID,
  resourceKey,
  typeBadge,
} from "../utils/graphBuilder";

export {
  NODE_W,
  NODE_H,
  ARROW_SIZE,
  buildGraph,
  edgePath,
  shouldZoomOnWheel,
  shouldActivateNode,
  parseProviderID,
  resourceKey,
  typeBadge,
};
export type { ImpactVisualStatus, GraphNode, GraphEdge };

interface DependencyGraphProps {
  answer?: ImpactAnswer | null;
  target?: ResourceIdentity | null;
  allRelationships?: DependencyRelationship[];
  onSelectNode?: (id: ResourceIdentity) => void;
}

export const DependencyGraph: React.FC<DependencyGraphProps> = ({
  answer,
  target,
  allRelationships,
  onSelectNode,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ mx: 0, my: 0, ox: 0, oy: 0 });
  const dragDistanceRef = useRef(0);
  const isDraggingRef = useRef(false);

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [showTable, setShowTable] = useState(false);

  const effectiveTarget = target || answer?.target || null;
  const isImpactMode = Boolean(
    answer &&
      (!effectiveTarget || resourceKey(answer.target) === resourceKey(effectiveTarget))
  );

  const { nodes, edges, width, height } = useMemo(() => {
    if (!effectiveTarget && !answer) {
      return { nodes: [], edges: [], width: 540, height: 260 };
    }
    return buildGraph(effectiveTarget || answer!, allRelationships, answer);
  }, [effectiveTarget, allRelationships, answer]);

  // Auto-fit to view
  const fitToView = useCallback(() => {
    if (!containerRef.current) return;
    const { clientWidth, clientHeight } = containerRef.current;
    if (width === 0 || height === 0 || clientWidth === 0 || clientHeight === 0) return;
    const scaleX = (clientWidth - 40) / width;
    const scaleY = (clientHeight - 40) / height;
    const s = Math.max(0.2, Math.min(scaleX, scaleY, 1.15));
    setScale(s);
    setOffset({
      x: Math.max(20, (clientWidth - width * s) / 2),
      y: Math.max(20, (clientHeight - height * s) / 2),
    });
  }, [width, height]);

  useEffect(() => {
    fitToView();
  }, [fitToView]);

  // Dragging (Pan)
  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    setDragging(true);
    isDraggingRef.current = true;
    dragDistanceRef.current = 0;
    setDragStart({ mx: e.clientX, my: e.clientY, ox: offset.x, oy: offset.y });
  };

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!isDraggingRef.current) return;
      const dx = e.clientX - dragStart.mx;
      const dy = e.clientY - dragStart.my;
      dragDistanceRef.current = updateMaxDragDistance(dragDistanceRef.current, dx, dy);
      setOffset({
        x: dragStart.ox + dx,
        y: dragStart.oy + dy,
      });
    },
    [dragStart]
  );

  const handleMouseUp = useCallback(() => {
    setDragging(false);
    isDraggingRef.current = false;
  }, []);

  useEffect(() => {
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [handleMouseMove, handleMouseUp]);

  // Opt-in modifier zoom (Ctrl/Meta only). Normal wheel gestures scroll the page!
  const handleWheel = (e: React.WheelEvent) => {
    if (shouldZoomOnWheel(e)) {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.9 : 1.1;
      setScale((s) => Math.max(0.15, Math.min(2.5, s * delta)));
    }
  };

  const handleNodeClick = (node: GraphNode) => {
    if (!shouldActivateNode(dragDistanceRef.current)) return;
    setSelectedNodeId(node.id);
    if (onSelectNode) {
      onSelectNode({
        provider: node.provider,
        resource_type: node.type,
        provider_id: node.providerId,
      });
    }
  };

  const nodeById = useMemo(() => {
    const m = new Map<string, GraphNode>();
    nodes.forEach((n) => m.set(n.id, n));
    return m;
  }, [nodes]);

  return (
    <div className="graph-section" style={{ position: "relative" }}>
      {/* ── Graph Header & Controls ── */}
      <div className="graph-header">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="graph-header-label">
            {isImpactMode ? "Impact Blast Radius Topology" : "Resource Topology & Relationships"}
          </span>
          <span
            className="risk-badge low"
            style={{
              fontSize: 9,
              padding: "1px 6px",
              background: isImpactMode ? "rgba(214,90,90,0.15)" : "rgba(255,255,255,0.08)",
              color: isImpactMode ? "var(--status-error)" : "var(--text-secondary)",
              border: isImpactMode ? "1px solid var(--status-error)" : "1px solid var(--border)",
            }}
          >
            {isImpactMode
              ? `SIMULATION: ${answer?.change_assessment?.change_type || "IMPACT"}`
              : "EXPLORATION MODE"}
          </span>
          <span
            style={{
              fontSize: 11,
              color: "var(--text-dim)",
              fontFamily: "var(--font-mono)",
            }}
          >
            {nodes.length} {nodes.length === 1 ? "node" : "nodes"} · {edges.length}{" "}
            {edges.length === 1 ? "edge" : "edges"}
          </span>
        </div>
        <div className="graph-controls">
          <button
            className="graph-ctrl-btn"
            onClick={fitToView}
            title="Fit graph to view"
            aria-label="Fit graph to view"
          >
            <Maximize2 size={12} /> Fit
          </button>
          <button
            className="graph-ctrl-btn"
            onClick={() => setScale((s) => Math.max(0.15, s - 0.15))}
            aria-label="Zoom out"
            title="Zoom out"
          >
            <Minus size={12} />
          </button>
          <button
            className="graph-ctrl-btn"
            onClick={() => setScale((s) => Math.min(2.5, s + 0.15))}
            aria-label="Zoom in"
            title="Zoom in"
          >
            <Plus size={12} />
          </button>
          <button
            className="graph-ctrl-btn"
            onClick={() => setScale(1)}
            aria-label="Reset zoom to 100%"
            title="Reset zoom to 100%"
          >
            <Minimize2 size={12} />
          </button>
          <button
            className="graph-accessible-toggle"
            onClick={() => setShowTable((v) => !v)}
            aria-pressed={showTable}
            title="Toggle between graphical topology and structured table"
          >
            {showTable ? (
              <Network size={12} style={{ marginRight: 4 }} />
            ) : (
              <Table size={12} style={{ marginRight: 4 }} />
            )}
            {showTable ? "Graph" : "Table"}
          </button>
        </div>
      </div>

      {showTable ? (
        /* Accessible structured table */
        <div style={{ padding: "12px", maxHeight: "360px", overflowY: "auto" }}>
          <table className="impact-table" style={{ width: "100%" }}>
            <thead>
              <tr>
                <th>Resource Name</th>
                <th>Type</th>
                <th>Namespace</th>
                <th>Cluster</th>
                <th>{isImpactMode ? "Impact Status" : "Topology Status"}</th>
                <th>Distance</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((n) => (
                <tr
                  key={n.id}
                  onClick={() => handleNodeClick(n)}
                  style={{ cursor: "pointer" }}
                >
                  <td className="cell-resource">
                    <span
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 11,
                        color:
                          n.status === "impacted"
                            ? "var(--status-error)"
                            : n.status === "non_impacted"
                            ? "var(--status-ok)"
                            : "var(--text-primary)",
                        fontWeight: n.isTarget ? 600 : 400,
                      }}
                      title={n.providerId}
                    >
                      {n.name}
                    </span>
                  </td>
                  <td className="type-label">{n.type}</td>
                  <td
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-secondary)",
                    }}
                  >
                    {n.namespace || "—"}
                  </td>
                  <td
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-dim)",
                    }}
                  >
                    {n.cluster || "—"}
                  </td>
                  <td>
                    {n.status === "target" ? (
                      <span
                        className="risk-badge low"
                        style={{
                          background: "rgba(255,255,255,0.08)",
                          color: "var(--text-primary)",
                          border: "1px solid var(--text-primary)",
                        }}
                      >
                        TARGET
                      </span>
                    ) : n.status === "impacted" ? (
                      <span className="risk-badge high">
                        {n.impactType || "IMPACTED"}
                      </span>
                    ) : n.status === "non_impacted" ? (
                      <span
                        className="risk-badge low"
                        style={{
                          background: "var(--status-ok-bg)",
                          color: "var(--status-ok)",
                          border: "1px solid var(--status-ok)",
                        }}
                      >
                        NOT IMPACTED
                      </span>
                    ) : n.status === "connected" ? (
                      <span
                        className="risk-badge low"
                        style={{
                          background: "rgba(255,255,255,0.05)",
                          color: "var(--text-secondary)",
                          border: "1px solid var(--border)",
                        }}
                      >
                        CONNECTED
                      </span>
                    ) : (
                      <span
                        className="risk-badge low"
                        style={{
                          background: "rgba(255,255,255,0.06)",
                          color: "var(--text-dim)",
                        }}
                      >
                        INCONCLUSIVE
                      </span>
                    )}
                  </td>
                  <td
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11,
                      color: "var(--text-secondary)",
                    }}
                  >
                    {n.isTarget ? "0 hops" : `${n.depth} ${n.depth === 1 ? "hop" : "hops"}`}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div
          ref={containerRef}
          className="graph-canvas"
          onMouseDown={handleMouseDown}
          onWheel={handleWheel}
          role="img"
          aria-label={`Impact dependency topology showing ${nodes.length} nodes and ${edges.length} edges`}
          style={{
            position: "relative",
            overflow: "hidden",
            cursor: dragging ? "grabbing" : "grab",
            height: "360px",
          }}
        >
          <div
            className="graph-canvas-inner"
            style={{
              transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
              transformOrigin: "0 0",
              width: width,
              height: height,
            }}
          >
            <svg
              width={width}
              height={height}
              style={{ overflow: "visible", display: "block" }}
              aria-hidden="true"
            >
              <defs>
                <marker
                  id="arrowhead-propagating"
                  markerWidth="7"
                  markerHeight="7"
                  refX="6"
                  refY="3.5"
                  orient="auto"
                >
                  <path d="M0,1 L0,6 L6,3.5 z" fill="var(--status-ok)" />
                </marker>
                <marker
                  id="arrowhead-boundary"
                  markerWidth="7"
                  markerHeight="7"
                  refX="6"
                  refY="3.5"
                  orient="auto"
                >
                  <path d="M0,1 L0,6 L6,3.5 z" fill="var(--text-dim)" />
                </marker>
              </defs>

              {/* ── Directed Edges ── */}
              {edges.map((e, i) => {
                const fromNode = nodeById.get(e.from);
                const toNode = nodeById.get(e.to);
                if (!fromNode || !toNode) return null;
                const d = edgePath(fromNode, toNode);
                const mx = (fromNode.x + toNode.x + NODE_W) / 2;
                const my = (fromNode.y + toNode.y + NODE_H) / 2;
                const isProp = e.isPropagating;

                return (
                  <g key={`edge-${i}`}>
                    <path
                      d={d}
                      className="graph-edge-line"
                      stroke={isProp ? "var(--status-ok)" : "var(--border-strong)"}
                      strokeWidth={isProp ? 1.5 : 1.2}
                      strokeDasharray={isProp ? undefined : "4,4"}
                      fill="none"
                      markerEnd={
                        isProp
                          ? "url(#arrowhead-propagating)"
                          : "url(#arrowhead-boundary)"
                      }
                      opacity={0.8}
                    />
                    <rect
                      x={mx - 30}
                      y={my - 8}
                      width={60}
                      height={15}
                      fill="var(--bg-panel)"
                      stroke="var(--border)"
                      strokeWidth={1}
                      rx={2}
                    />
                    <text
                      x={mx}
                      y={my + 3}
                      className="graph-edge-label"
                      textAnchor="middle"
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 8.5,
                        fill: isProp ? "var(--status-ok)" : "var(--text-dim)",
                        letterSpacing: "0.04em",
                      }}
                    >
                      {e.label.toUpperCase()}
                    </text>
                  </g>
                );
              })}

              {/* ── Graph Nodes ── */}
              {nodes.map((node) => {
                const isSelected = selectedNodeId === node.id;
                const truncatedName =
                  node.name.length > 18 ? node.name.slice(0, 17) + "…" : node.name;
                const subMeta = node.namespace
                  ? `${node.namespace} · ${node.type}`
                  : node.type;
                const truncatedSub =
                  subMeta.length > 21 ? subMeta.slice(0, 20) + "…" : subMeta;

                // Honest Impact-Status Coloring
                const borderColor = isSelected
                  ? "var(--text-primary)"
                  : node.status === "target"
                  ? "var(--text-primary)"
                  : node.status === "impacted"
                  ? "var(--status-error)"
                  : node.status === "non_impacted"
                  ? "var(--status-ok)"
                  : node.status === "connected"
                  ? "var(--border-strong)"
                  : "var(--text-dim)";

                const borderWidth = isSelected || node.status === "target" ? 2 : 1.5;

                const badgeBg =
                  node.status === "target"
                    ? "rgba(255, 255, 255, 0.1)"
                    : node.status === "impacted"
                    ? "var(--status-err-bg)"
                    : node.status === "non_impacted"
                    ? "var(--status-ok-bg)"
                    : node.status === "connected"
                    ? "rgba(255, 255, 255, 0.05)"
                    : "rgba(255, 255, 255, 0.04)";

                const badgeTextFill =
                  node.status === "target"
                    ? "var(--text-primary)"
                    : node.status === "impacted"
                    ? "var(--status-error)"
                    : node.status === "non_impacted"
                    ? "var(--status-ok)"
                    : node.status === "connected"
                    ? "var(--text-secondary)"
                    : "var(--text-dim)";

                const tooltipStatus =
                  node.status === "target"
                    ? "TARGET (Selected resource)"
                    : node.status === "connected"
                    ? "CONNECTED (Topological neighbor — impact not simulated)"
                    : node.status === "impacted"
                    ? "IMPACTED (Identified as potentially broken by backend analysis)"
                    : node.status === "non_impacted"
                    ? "NOT IMPACTED (Not identified as impacted by complete analysis)"
                    : "INCONCLUSIVE (Status inconclusive due to partial analysis)";

                return (
                  <g
                    key={node.id}
                    className="graph-node-group"
                    transform={`translate(${node.x}, ${node.y})`}
                    onClick={() => handleNodeClick(node)}
                    style={{ cursor: "pointer" }}
                    role="button"
                    aria-label={`${node.name} (${node.type}) — ${tooltipStatus}`}
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleNodeClick(node);
                    }}
                  >
                    <title>{`${node.providerId} (${node.type})\nCluster: ${node.cluster || "default"}\nNamespace: ${node.namespace || "(cluster-scoped)"}\nStatus: ${tooltipStatus}`}</title>

                    {/* Node background rect */}
                    <rect
                      width={NODE_W}
                      height={NODE_H}
                      rx={3}
                      ry={3}
                      fill="var(--bg-elevated)"
                      stroke={borderColor}
                      strokeWidth={borderWidth}
                    />

                    {/* Left type badge container */}
                    <path
                      d={`M 0 3 Q 0 0 3 0 L 32 0 L 32 ${NODE_H} L 3 ${NODE_H} Q 0 ${NODE_H} 0 ${NODE_H - 3} Z`}
                      fill={badgeBg}
                    />

                    {/* Type code */}
                    <text
                      x={16}
                      y={NODE_H / 2 + 1}
                      textAnchor="middle"
                      dominantBaseline="middle"
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 9,
                        fontWeight: 700,
                        fill: badgeTextFill,
                      }}
                    >
                      {typeBadge(node.type)}
                    </text>

                    {/* Vertical separator */}
                    <line
                      x1={32}
                      y1={0}
                      x2={32}
                      y2={NODE_H}
                      stroke="var(--border)"
                      strokeWidth={1}
                    />

                    {/* Resource name and namespace */}
                    <text
                      x={40}
                      y={22}
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 11,
                        fontWeight: node.isTarget ? 600 : 500,
                        fill:
                          node.status === "impacted"
                            ? "var(--status-error)"
                            : "var(--text-primary)",
                      }}
                    >
                      {truncatedName}
                    </text>
                    <text
                      x={40}
                      y={40}
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 10,
                        fill: "var(--text-dim)",
                      }}
                    >
                      {truncatedSub}
                    </text>
                  </g>
                );
              })}
            </svg>
          </div>
        </div>
      )}

      {/* ── Fixed Visual Legend (Bottom of Investigate Graph) ── */}
      <div
        style={{
          borderTop: "1px solid var(--border)",
          background: "var(--bg-panel)",
          padding: "8px 14px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          flexWrap: "wrap",
          gap: 12,
          fontSize: 11,
          fontFamily: "var(--font-mono)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 14, flexWrap: "wrap" }}>
          <span style={{ color: "var(--text-dim)", textTransform: "uppercase", fontSize: 10 }}>
            {isImpactMode ? "Simulation States:" : "Topology States:"}
          </span>
          <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
            <span
              style={{
                width: 10,
                height: 10,
                border: "2px solid var(--text-primary)",
                background: "#181818",
                display: "inline-block",
                borderRadius: 2,
              }}
            />
            Target
          </span>
          {isImpactMode ? (
            <>
              <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
                <span
                  style={{
                    width: 10,
                    height: 10,
                    border: "1.5px solid var(--status-error)",
                    background: "rgba(214,90,90,0.15)",
                    display: "inline-block",
                    borderRadius: 2,
                  }}
                />
                <strong style={{ color: "var(--status-error)" }}>Potentially Impacted</strong>
              </span>
              <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
                <span
                  style={{
                    width: 10,
                    height: 10,
                    border: "1.5px solid var(--status-ok)",
                    background: "rgba(85,200,120,0.15)",
                    display: "inline-block",
                    borderRadius: 2,
                  }}
                />
                <span style={{ color: "var(--status-ok)" }}>Not identified as impacted</span>
              </span>
              <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
                <span
                  style={{
                    width: 10,
                    height: 10,
                    border: "1px solid var(--text-dim)",
                    background: "var(--bg-elevated)",
                    display: "inline-block",
                    borderRadius: 2,
                  }}
                />
                <span style={{ color: "var(--text-dim)" }}>Status Inconclusive</span>
              </span>
            </>
          ) : (
            <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
              <span
                style={{
                  width: 10,
                  height: 10,
                  border: "1.5px solid var(--border-strong)",
                  background: "var(--bg-elevated)",
                  display: "inline-block",
                  borderRadius: 2,
                }}
              />
              <span style={{ color: "var(--text-secondary)" }}>Connected Dependency (Neutral)</span>
            </span>
          )}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <svg width={22} height={10}>
              <line x1={0} y1={5} x2={16} y2={5} stroke="var(--status-ok)" strokeWidth={2} />
              <polygon points="16,5 12,2 12,8" fill="var(--status-ok)" />
            </svg>
            <span style={{ color: "var(--text-secondary)", fontSize: 10 }}>
              Impact-propagating (DEPENDS_ON)
            </span>
          </span>
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <svg width={22} height={10}>
              <line
                x1={0}
                y1={5}
                x2={16}
                y2={5}
                stroke="var(--text-dim)"
                strokeWidth={1.2}
                strokeDasharray="3,3"
              />
              <polygon points="16,5 12,2 12,8" fill="var(--text-dim)" />
            </svg>
            <span style={{ color: "var(--text-dim)", fontSize: 10 }}>
              Lifecycle boundary (OWNS)
            </span>
          </span>
        </div>
      </div>
    </div>
  );
};
