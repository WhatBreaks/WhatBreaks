import test from "node:test";
import assert from "node:assert/strict";
import { sameResourceIdentity } from "./resource.ts";
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
