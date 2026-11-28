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

test("ResourceInventory renders fallback time for empty or invalid timestamps", () => {
  const invalidTimeResources: ResourceItem[] = [
    {
      provider: "kubernetes",
      resource_type: "pod",
      provider_id: "cluster/default/pod-1",
      first_observed_at: "",
      last_observed_at: "invalid-date",
    },
  ];

  const markup = renderToStaticMarkup(
    createElement(ResourceInventory, {
      resources: invalidTimeResources,
      selectedResource: null,
      onSelectResource: () => {},
      loading: false,
      error: null,
      onRefresh: () => {},
    }),
  );

  assert.ok(markup.includes("--:--:--"), "renders graceful placeholder time");
  assert.ok(!markup.includes("Invalid Date"), "does not render raw Invalid Date string");
});

test("ResourceInventory displays empty state when no resources match", () => {
  const markup = renderToStaticMarkup(
    createElement(ResourceInventory, {
      resources: [],
      selectedResource: null,
      onSelectResource: () => {},
      loading: false,
      error: null,
      onRefresh: () => {},
    }),
  );

  assert.ok(markup.includes("No Resources Discovered"));
});
