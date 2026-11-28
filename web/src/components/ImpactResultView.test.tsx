import test from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ImpactResultView } from "./ImpactResultView.tsx";
import type { ImpactAnswer } from "../types.ts";

const baseAnswer: ImpactAnswer = {
  target: {
    provider: "kubernetes",
    resource_type: "deployment",
    provider_id: "cluster-a/default/api-gateway",
  },
  summary: {
    impacted_count: 1,
    direct_count: 1,
    indirect_count: 0,
    max_depth: 1,
  },
  impacted_resources: [
    {
      resource: {
        provider: "kubernetes",
        resource_type: "service",
        provider_id: "cluster-a/default/api-service",
      },
      depth: 1,
      impact_type: "BREAKAGE_RISK",
      impact_reason: "Direct dependency",
    },
  ],
  relationships: [],
  paths: [],
  evidence: [],
  explanation_facts: [],
};

test("ImpactResultView renders safely without crashing when assumptions or limitations are omitted", () => {
  const answerWithoutDisclosures: ImpactAnswer = {
    ...baseAnswer,
    change_assessment: {
      change_type: "DELETE",
      impact_nature: "Resource removal",
    },
  };

  const markup = renderToStaticMarkup(
    createElement(ImpactResultView, {
      answer: answerWithoutDisclosures,
      error: null,
      loading: false,
    }),
  );

  assert.ok(markup.includes("Engine Assumptions"));
  assert.ok(markup.includes("Engine Limitations"));
  assert.ok(markup.includes("None stated"));
  assert.ok(markup.includes("Blast Radius Analysis Results"));
});

test("ImpactResultView renders populated assumptions and limitations lists", () => {
  const answerWithDisclosures: ImpactAnswer = {
    ...baseAnswer,
    change_assessment: {
      change_type: "UPDATE",
      impact_nature: "Config update",
      assumptions: ["Pods restart in order"],
      limitations: ["Dynamic endpoints unmonitored"],
    },
  };

  const markup = renderToStaticMarkup(
    createElement(ImpactResultView, {
      answer: answerWithDisclosures,
      error: null,
      loading: false,
    }),
  );

  assert.ok(markup.includes("Pods restart in order"));
  assert.ok(markup.includes("Dynamic endpoints unmonitored"));
});

test("ImpactResultView renders loading, error, and empty states gracefully", () => {
  const loadingMarkup = renderToStaticMarkup(
    createElement(ImpactResultView, {
      answer: null,
      error: null,
      loading: true,
    }),
  );
  assert.ok(loadingMarkup.includes("Traversing Dependency Graph"));

  const errorMarkup = renderToStaticMarkup(
    createElement(ImpactResultView, {
      answer: null,
      error: "Connection refused",
      loading: false,
    }),
  );
  assert.ok(errorMarkup.includes("Analysis Execution Failure"));
  assert.ok(errorMarkup.includes("Connection refused"));

  const emptyMarkup = renderToStaticMarkup(
    createElement(ImpactResultView, {
      answer: null,
      error: null,
      loading: false,
    }),
  );
  assert.ok(emptyMarkup.includes("No Analysis Executed Yet"));
});
