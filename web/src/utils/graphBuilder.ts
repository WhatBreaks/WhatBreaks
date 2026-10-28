import type { ImpactAnswer, ResourceIdentity, DependencyRelationship } from "../types.ts";
import {
  parseProviderID,
  resourceKey,
  typeBadge,
  isImpactPropagating,
} from "./resource.ts";

export { parseProviderID, resourceKey, typeBadge, isImpactPropagating };

// ── Layout constants ──────────────────────────────────────────────────────────
export const NODE_W = 210;
export const NODE_H = 58;
export const COL_GAP = 130;
export const ROW_GAP = 28;
export const PADDING = 36;
export const ARROW_SIZE = 6;
export const DRAG_THRESHOLD = 5;

/**
 * Track the greatest distance from the original mouse-down position.
 * dx and dy must be measured relative to that same origin for each event.
 */
export function updateMaxDragDistance(maxDistance: number, dx: number, dy: number): number {
  return Math.max(maxDistance, Math.hypot(dx, dy));
}

export type ImpactVisualStatus = "target" | "impacted" | "non_impacted" | "inconclusive" | "connected";

export interface GraphNode {
  id: string;
  provider: string;
  type: string;
  providerId: string;
  cluster: string;
  namespace: string;
  name: string;
  depth: number;
  x: number;
  y: number;
  isTarget: boolean;
  isImpacted: boolean;
  status: ImpactVisualStatus;
  impactType?: string;
  impactReason?: string;
}

export interface GraphEdge {
  id: string;
  from: string;
  to: string;
  label: string;
  kind: string;
  isPropagating: boolean;
}

export interface WheelLikeEvent {
  ctrlKey?: boolean;
  metaKey?: boolean;
}

/**
 * Checks if a wheel event should trigger graph zoom.
 * Normal wheel/trackpad gestures without modifier keys MUST scroll the page,
 * preventing mouse wheel scrolling from being hijacked.
 */
export function shouldZoomOnWheel(e: WheelLikeEvent): boolean {
  return Boolean(e.ctrlKey || e.metaKey);
}

/**
 * Checks whether mouse interaction should activate node selection.
 * If user dragged more than DRAG_THRESHOLD (5px) to pan, node activation is suppressed.
 */
export function shouldActivateNode(dragDistance: number): boolean {
  return dragDistance <= DRAG_THRESHOLD;
}

/**
 * Builds the connected topology graph for Investigate:
 * - Target resource (status: "target", highlighted)
 * - In Exploration mode (no ImpactAnswer):
 *   Connected resources are displayed immediately with status: "connected" (neutral, never falsely marked as impacted)
 * - In Impact Analysis mode (with ImpactAnswer):
 *   Backend-identified potentially impacted resources (status: "impacted", red)
 *   Relevant connected resources not identified as impacted (status: "non_impacted", green)
 *   If analysis was partial/inconclusive, marked as status: "inconclusive" (gray)
 * - Directed edges with propagation semantics: isPropagating true for DEPENDS_ON/CALLS/ROUTES_TO,
 *   false for OWNS/RUNS_ON/LIFECYCLE boundaries.
 */
export function buildGraph(
  targetOrAnswer: ImpactAnswer | ResourceIdentity,
  allRelationships?: DependencyRelationship[],
  optionalAnswer?: ImpactAnswer | null
): { nodes: GraphNode[]; edges: GraphEdge[]; width: number; height: number } {
  let target: ResourceIdentity;
  let answer: ImpactAnswer | null = null;

  if (
    typeof targetOrAnswer === "object" &&
    targetOrAnswer !== null &&
    "impacted_resources" in targetOrAnswer &&
    "target" in targetOrAnswer
  ) {
    answer = targetOrAnswer as ImpactAnswer;
    target = answer.target;
  } else {
    target = targetOrAnswer as ResourceIdentity;
    answer = optionalAnswer ?? null;
  }

  // Safety Invalidation: If answer exists but belongs to a different target, invalidate it
  // to avoid displaying stale impact results on the new selection!
  if (answer && resourceKey(answer.target) !== resourceKey(target)) {
    answer = null;
  }

  const nodeMap = new Map<string, GraphNode>();
  const impactedMap = new Map<string, { depth: number; type?: string; reason?: string }>();

  if (answer) {
    for (const ir of answer.impacted_resources) {
      impactedMap.set(resourceKey(ir.resource), {
        depth: ir.depth,
        type: ir.impact_type,
        reason: ir.impact_reason,
      });
    }
  }

  const targetKey = resourceKey(target);

  // Analysis completeness & conclusiveness check
  const isAnalysisConclusive = Boolean(
    answer
      ? answer.completeness
        ? answer.completeness.is_complete &&
          answer.completeness.status === "COMPLETE" &&
          (!answer.completeness.reasons || answer.completeness.reasons.length === 0)
        : true
      : true
  );

  const mkNode = (r: ResourceIdentity, isTarget = false): GraphNode => {
    const k = resourceKey(r);
    const parsed = parseProviderID(r.provider_id);
    const imp = impactedMap.get(k);

    let status: ImpactVisualStatus = "non_impacted";
    if (isTarget) {
      status = "target";
    } else if (answer) {
      if (imp) {
        status = "impacted";
      } else if (!isAnalysisConclusive) {
        status = "inconclusive";
      } else {
        status = "non_impacted";
      }
    } else {
      // Exploration mode: no change has been simulated, so connected nodes are neutral connected dependencies
      status = "connected";
    }

    return {
      id: k,
      provider: r.provider,
      type: r.resource_type,
      providerId: r.provider_id,
      cluster: parsed.cluster,
      namespace: parsed.namespace,
      name: parsed.name,
      depth: isTarget ? 0 : imp ? imp.depth : 1,
      x: 0,
      y: 0,
      isTarget,
      isImpacted: Boolean(imp && !isTarget),
      status,
      impactType: imp?.type,
      impactReason: imp?.reason,
    };
  };

  // 1. Register target node
  nodeMap.set(targetKey, mkNode(target, true));

  // 2. Register backend-identified impacted resources if in impact analysis mode
  if (answer) {
    for (const ir of answer.impacted_resources) {
      const k = resourceKey(ir.resource);
      if (!nodeMap.has(k)) {
        nodeMap.set(k, mkNode(ir.resource));
      }
    }
  }

  // 3. Register relevant connected topology neighborhood from allRelationships
  // Only add resources connected directly to target or any impacted node
  if (allRelationships && allRelationships.length > 0) {
    const focalKeys = new Set<string>([targetKey]);
    if (answer) {
      for (const ir of answer.impacted_resources) {
        focalKeys.add(resourceKey(ir.resource));
      }
    }

    for (const rel of allRelationships) {
      const sk = resourceKey(rel.source);
      const tk = resourceKey(rel.target);
      if (focalKeys.has(sk) || focalKeys.has(tk)) {
        if (!nodeMap.has(sk)) nodeMap.set(sk, mkNode(rel.source));
        if (!nodeMap.has(tk)) nodeMap.set(tk, mkNode(rel.target));
      }
    }
  }

  // 4. Edges from answer.relationships
  const edgeMap = new Map<string, GraphEdge>();
  if (answer && answer.relationships) {
    for (const rel of answer.relationships) {
      const sk = resourceKey(rel.relationship.source);
      const tk = resourceKey(rel.relationship.target);
      const kind = rel.relationship.kind;
      const ek = `${sk}->${tk}:${kind}`;
      if (!nodeMap.has(sk)) nodeMap.set(sk, mkNode(rel.relationship.source));
      if (!nodeMap.has(tk)) nodeMap.set(tk, mkNode(rel.relationship.target));
      edgeMap.set(ek, {
        id: ek,
        from: sk,
        to: tk,
        label: kind,
        kind: kind,
        isPropagating: isImpactPropagating(kind),
      });
    }
  }

  // 5. Connect all nodes in nodeMap using allRelationships
  if (allRelationships) {
    for (const rel of allRelationships) {
      const sk = resourceKey(rel.source);
      const tk = resourceKey(rel.target);
      if (nodeMap.has(sk) && nodeMap.has(tk)) {
        const kind = rel.kind;
        const ek = `${sk}->${tk}:${kind}`;
        if (!edgeMap.has(ek)) {
          edgeMap.set(ek, {
            id: ek,
            from: sk,
            to: tk,
            label: kind,
            kind: kind,
            isPropagating: isImpactPropagating(kind),
          });
        }
      }
    }
  }

  const edgeList = Array.from(edgeMap.values());

  // 6. Assign columns: target = 0, dependents to the right (+1, +2), dependencies to the left (-1)
  const colMap = new Map<string, number>();
  colMap.set(targetKey, 0);

  // Directly set column for impacted nodes based on their verified hop depth
  for (const [id, node] of nodeMap) {
    if (node.isTarget) continue;
    if (impactedMap.has(id)) {
      colMap.set(id, impactedMap.get(id)!.depth);
    }
  }

  // Infer layer for connected nodes relative to target
  for (const edge of edgeList) {
    if (edge.from === targetKey && !colMap.has(edge.to)) {
      colMap.set(edge.to, -1);
    }
    if (edge.to === targetKey && !colMap.has(edge.from)) {
      colMap.set(edge.from, 1);
    }
  }

  // For any remaining nodes, BFS assignment
  const visited = new Set<string>([targetKey]);
  const queue: string[] = [targetKey];

  const downstream = new Map<string, Set<string>>();
  const upstream = new Map<string, Set<string>>();
  for (const e of edgeList) {
    if (!downstream.has(e.from)) downstream.set(e.from, new Set());
    if (!upstream.has(e.to)) upstream.set(e.to, new Set());
    downstream.get(e.from)!.add(e.to);
    upstream.get(e.to)!.add(e.from);
  }

  while (queue.length > 0) {
    const cur = queue.shift()!;
    const curCol = colMap.get(cur) ?? 0;

    for (const d of downstream.get(cur) ?? []) {
      if (!colMap.has(d)) {
        colMap.set(d, curCol + 1);
      }
      if (!visited.has(d)) {
        visited.add(d);
        queue.push(d);
      }
    }
    for (const u of upstream.get(cur) ?? []) {
      if (!colMap.has(u)) {
        colMap.set(u, curCol - 1);
      }
      if (!visited.has(u)) {
        visited.add(u);
        queue.push(u);
      }
    }
  }

  // Ensure every node has a column
  for (const [id] of nodeMap) {
    if (!colMap.has(id)) {
      colMap.set(id, 0);
    }
  }

  // Normalize columns so minCol starts at 0
  const allCols = [...colMap.values()];
  const minCol = Math.min(0, ...allCols);

  const byCol = new Map<number, string[]>();
  for (const [id, c] of colMap) {
    const norm = c - minCol;
    if (!byCol.has(norm)) byCol.set(norm, []);
    byCol.get(norm)!.push(id);
  }

  // Sort nodes within each column deterministically
  const sortedColKeys = [...byCol.keys()].sort((a, b) => a - b);
  const maxRowsInCol = Math.max(1, ...[...byCol.values()].map((v) => v.length));

  for (const c of sortedColKeys) {
    const ids = byCol.get(c)!;
    ids.sort((a, b) => {
      const na = nodeMap.get(a)!;
      const nb = nodeMap.get(b)!;
      if (na.isTarget) return -1;
      if (nb.isTarget) return 1;
      if (na.isImpacted !== nb.isImpacted) {
        return na.isImpacted ? -1 : 1;
      }
      return na.name.localeCompare(nb.name);
    });

    const colHeight = ids.length * (NODE_H + ROW_GAP) - ROW_GAP;
    const totalHeight = maxRowsInCol * (NODE_H + ROW_GAP) - ROW_GAP;
    const yOffset = Math.max(0, (totalHeight - colHeight) / 2);

    ids.forEach((id, rowIdx) => {
      const node = nodeMap.get(id)!;
      node.x = PADDING + c * (NODE_W + COL_GAP);
      node.y = PADDING + yOffset + rowIdx * (NODE_H + ROW_GAP);
    });
  }

  const allNodes = [...nodeMap.values()];
  const maxX = allNodes.reduce((m, n) => Math.max(m, n.x + NODE_W), 0) + PADDING;
  const maxY = allNodes.reduce((m, n) => Math.max(m, n.y + NODE_H), 0) + PADDING;

  return {
    nodes: allNodes,
    edges: edgeList,
    width: Math.max(maxX, 540),
    height: Math.max(maxY, 260),
  };
}

export function edgePath(from: GraphNode, to: GraphNode): string {
  if (from.x < to.x) {
    const x1 = from.x + NODE_W;
    const y1 = from.y + NODE_H / 2;
    const x2 = to.x;
    const y2 = to.y + NODE_H / 2;
    const cx = (x1 + x2) / 2;
    return `M ${x1} ${y1} C ${cx} ${y1}, ${cx} ${y2}, ${x2 - ARROW_SIZE} ${y2}`;
  }

  if (from.x > to.x) {
    const x1 = from.x;
    const y1 = from.y + NODE_H / 2;
    const x2 = to.x + NODE_W;
    const y2 = to.y + NODE_H / 2;
    const cx = (x1 + x2) / 2;
    return `M ${x1} ${y1} C ${cx} ${y1}, ${cx} ${y2}, ${x2 + ARROW_SIZE} ${y2}`;
  }

  const x1 = from.x + NODE_W;
  const y1 = from.y + NODE_H / 2;
  const x2 = to.x + NODE_W;
  const y2 = to.y + NODE_H / 2;
  const loopX = x1 + 32;
  return `M ${x1} ${y1} C ${loopX} ${y1}, ${loopX} ${y2}, ${x2 + ARROW_SIZE} ${y2}`;
}
