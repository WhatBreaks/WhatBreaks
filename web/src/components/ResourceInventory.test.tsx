import test from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ResourceInventory } from "./ResourceInventory.tsx";
import type { ResourceItem } from "../types.ts";

const resources: ResourceItem[] = [
  {
    provider: "kubernetes",
    resource_type: "service",
    provider_id: "shared-resource-id",
    first_observed_at: "2026-10-28T00:00:00.000Z",
    last_observed_at: "2026-10-28T00:01:00.000Z",
  },
  {
    provider: "terraform",
    resource_type: "service",
    provider_id: "shared-resource-id",
    first_observed_at: "2026-10-28T00:00:00.000Z",
    last_observed_at: "2026-10-28T00:01:00.000Z",
  },
];

test("ResourceInventory selects only the resource with matching provider identity", () => {
  const markup = renderToStaticMarkup(
    createElement(ResourceInventory, {
      resources,
      selectedResource: resources[0],
      onSelectResource: () => {},
      loading: false,
      error: null,
      onRefresh: () => {},
    }),
  );

  const cards = [...markup.matchAll(/<div class="resource-card( selected)?\s*"/g)];

  assert.equal(cards.length, 2, "both resources should be rendered");
  assert.equal(cards[0][1], " selected", "the matching Kubernetes resource should be selected");
  assert.equal(cards[1][1], undefined, "the Terraform resource should not be selected");
});
