import test from "node:test";
import assert from "node:assert/strict";
import { sameResourceIdentity, parseProviderID } from "./resource.ts";
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

test("parseProviderID parses namespaced, cluster-scoped, and trimmed IDs correctly", () => {
  assert.deepEqual(parseProviderID("k3d-cluster/default/web-service"), {
    cluster: "k3d-cluster",
    namespace: "default",
    name: "web-service",
  });

  // Cluster-scoped resource (empty namespace)
  assert.deepEqual(parseProviderID("k3d-cluster//node-1"), {
    cluster: "k3d-cluster",
    namespace: "",
    name: "node-1",
  });

  // Two-segment cluster/name format
  assert.deepEqual(parseProviderID("k3d-cluster/node-1"), {
    cluster: "k3d-cluster",
    namespace: "",
    name: "node-1",
  });

  // Leading and trailing whitespace / slashes
  assert.deepEqual(parseProviderID("  /k3d-cluster/default/pod-1/  "), {
    cluster: "k3d-cluster",
    namespace: "default",
    name: "pod-1",
  });

  // Multi-segment resource name
  assert.deepEqual(parseProviderID("cluster/ns/group/resource/sub"), {
    cluster: "cluster",
    namespace: "ns",
    name: "group/resource/sub",
  });

  // Empty or invalid input
  assert.deepEqual(parseProviderID(""), { cluster: "", namespace: "", name: "" });
  assert.deepEqual(parseProviderID("   "), { cluster: "", namespace: "", name: "" });
  assert.deepEqual(parseProviderID("///"), { cluster: "", namespace: "", name: "" });
});
