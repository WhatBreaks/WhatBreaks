import test from "node:test";
import assert from "node:assert/strict";
import { sameResourceIdentity, typeBadge } from "./resource.ts";
import type { ResourceIdentity } from "../types.ts";

const kubernetesService: ResourceIdentity = {
  provider: "kubernetes",
  resource_type: "service",
  provider_id: "shared-resource-id",
};

const anotherProviderService: ResourceIdentity = {
  provider: "terraform",
  resource_type: "service",
  provider_id: "shared-resource-id",
};

test("resource identity matching includes provider, type, and provider ID", () => {
  assert.equal(sameResourceIdentity(kubernetesService, { ...kubernetesService }), true);
  assert.equal(sameResourceIdentity(kubernetesService, anotherProviderService), false);
  assert.equal(
    sameResourceIdentity(kubernetesService, { ...kubernetesService, resource_type: "pod" }),
    false,
  );
  assert.equal(
    sameResourceIdentity(kubernetesService, { ...kubernetesService, provider_id: "another-id" }),
    false,
  );
  assert.equal(sameResourceIdentity(null, kubernetesService), false);
  assert.equal(sameResourceIdentity(kubernetesService, undefined), false);
});

test("typeBadge maps common Kubernetes resource types to canonical abbreviations", () => {
  assert.equal(typeBadge("Deployment"), "DEP");
  assert.equal(typeBadge("StatefulSet"), "STS");
  assert.equal(typeBadge("DaemonSet"), "DS");
  assert.equal(typeBadge("Service"), "SVC");
  assert.equal(typeBadge("ConfigMap"), "CM");
  assert.equal(typeBadge("Secret"), "SEC");
  assert.equal(typeBadge("Ingress"), "ING");
  assert.equal(typeBadge("IngressClass"), "INGC");
  assert.equal(typeBadge("Pod"), "POD");
  assert.equal(typeBadge("Node"), "NOD");
  assert.equal(typeBadge("PersistentVolumeClaim"), "PVC");
  assert.equal(typeBadge("PersistentVolume"), "PV");
  assert.equal(typeBadge("ReplicaSet"), "RS");
  assert.equal(typeBadge("ServiceAccount"), "SA");
  assert.equal(typeBadge("Job"), "JOB");
  assert.equal(typeBadge("CronJob"), "CRON");
  assert.equal(typeBadge("Namespace"), "NS");
  assert.equal(typeBadge("HorizontalPodAutoscaler"), "HPA");
  assert.equal(typeBadge("NetworkPolicy"), "NP");
  assert.equal(typeBadge("CustomResourceDefinition"), "CRD");
  assert.equal(typeBadge("StorageClass"), "SC");
  assert.equal(typeBadge("ClusterRole"), "CR");
  assert.equal(typeBadge("ClusterRoleBinding"), "CRB");
  assert.equal(typeBadge("RoleBinding"), "RB");
  assert.equal(typeBadge("Role"), "ROLE");
  assert.equal(typeBadge("Endpoints"), "EP");
  assert.equal(typeBadge("EndpointSlice"), "EP");

  // Short codes and case-insensitivity
  assert.equal(typeBadge("ns"), "NS");
  assert.equal(typeBadge("hpa"), "HPA");
  assert.equal(typeBadge("crd"), "CRD");
  assert.equal(typeBadge("sc"), "SC");

  // Fallback and empty
  assert.equal(typeBadge("CustomApp"), "CUS");
  assert.equal(typeBadge(""), "");
});
