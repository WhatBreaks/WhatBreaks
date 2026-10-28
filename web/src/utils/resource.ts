import type { ResourceIdentity } from "../types";

/**
 * Parses a provider ID (e.g., "k3d-license-service/license-system/license-service")
 * into its cluster, namespace, and name components.
 */
export function parseProviderID(id: string): { cluster: string; namespace: string; name: string } {
  if (!id) {
    return { cluster: "", namespace: "", name: "" };
  }
  const parts = id.split("/");
  if (parts.length >= 3) {
    return { cluster: parts[0], namespace: parts[1], name: parts.slice(2).join("/") };
  }
  if (parts.length === 2) {
    return { cluster: parts[0], namespace: "", name: parts[1] };
  }
  return { cluster: "", namespace: "", name: id };
}

/**
 * Computes the unique canonical key for a resource identity.
 */
export function resourceKey(r: ResourceIdentity): string {
  return `${r.provider}/${r.resource_type}/${r.provider_id}`;
}

/**
 * Checks whether two resource identities refer to the same provider, type, and ID.
 */
export function sameResourceIdentity(
  left: ResourceIdentity | null | undefined,
  right: ResourceIdentity | null | undefined,
): boolean {
  return Boolean(left && right && resourceKey(left) === resourceKey(right));
}

/**
 * Returns a 2-3 letter type badge for compact terminal display.
 */
export function typeBadge(t: string): string {
  const lower = t.toLowerCase();
  if (lower.startsWith("deploy")) return "DEP";
  if (lower.startsWith("stateful")) return "STS";
  if (lower.startsWith("daemon")) return "DS";
  if (lower.startsWith("serv") && !lower.startsWith("serviceaccount")) return "SVC";
  if (lower.startsWith("config")) return "CM";
  if (lower.startsWith("sec")) return "SEC";
  if (lower.startsWith("ing")) return "ING";
  if (lower.startsWith("pod")) return "POD";
  if (lower.startsWith("node")) return "NOD";
  if (lower.startsWith("pvc") || lower.startsWith("persistentvolumeclaim")) return "PVC";
  if (lower.startsWith("pv") || lower.startsWith("persistentvolume")) return "PV";
  if (lower.startsWith("replicaset")) return "RS";
  if (lower.startsWith("serviceaccount")) return "SA";
  if (lower.startsWith("job")) return "JOB";
  if (lower.startsWith("cron")) return "CRON";
  return t.slice(0, 3).toUpperCase();
}

/**
 * Determines whether a relationship kind propagates runtime impact under WhatBreaks policy.
 * Only DEPENDS_ON, CALLS, READS_FROM, WRITES_TO propagate failure.
 * OWNS, RUNS_ON, AUTHORIZES are lifecycle/management boundaries only.
 */
export function isImpactPropagating(kind: string): boolean {
  const k = kind.toUpperCase();
  return k === "DEPENDS_ON" || k === "CALLS" || k === "READS_FROM" || k === "WRITES_TO";
}
