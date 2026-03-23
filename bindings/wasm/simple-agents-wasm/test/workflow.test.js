import test from "node:test";
import assert from "node:assert/strict";

import { Client } from "../index.js";

function makeJsonResponse(content) {
  return new Response(
    JSON.stringify({
      id: "cmpl_workflow",
      model: "gpt-4o-mini",
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
      choices: [{ finish_reason: "stop", message: { role: "assistant", content } }]
    }),
    { status: 200, headers: { "Content-Type": "application/json" } }
  );
}

test("runWorkflowYamlString executes set/call_function/if/output", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse("unused")
  });

  const yaml = `
version: "1"
steps:
  - id: set_title
    type: set
    key: title
    value: "YamSLAM Playground"

  - id: make_slug
    type: call_function
    function: slugify
    args:
      input: "{{title}}"

  - id: branch
    type: if
    condition:
      left: "{{make_slug}}"
      operator: contains
      right: "yamslam"
    then: ok
    else: bad

  - id: bad
    type: output
    text: "bad={{make_slug}}"

  - id: ok
    type: output
    text: "ok={{make_slug}}"
`;

  const result = await client.runWorkflowYamlString(
    yaml,
    { model: "gpt-4o-mini" },
    {
      functions: {
        slugify: ({ input }) => String(input).toLowerCase().replace(/\s+/g, "-")
      }
    }
  );

  assert.equal(result.status, "ok");
  assert.equal(result.context.make_slug, "yamslam-playground");
  assert.equal(result.output, "ok=yamslam-playground");
  assert.ok(result.events.length >= 4);
});

test("runWorkflowYamlString executes llm_call with workflow model", async () => {
  let requestBody;
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async (_url, init) => {
      requestBody = JSON.parse(init.body);
      return makeJsonResponse("hello from model");
    }
  });

  const yaml = `
model: gpt-4o-mini
steps:
  - id: ask
    type: llm_call
    prompt: "Say hi to {{name}}"
  - id: out
    type: output
    text: "{{ask}}"
`;

  const result = await client.runWorkflowYamlString(yaml, { name: "builder" });

  assert.equal(requestBody.model, "gpt-4o-mini");
  assert.equal(requestBody.messages[0].content, "Say hi to builder");
  assert.equal(result.output, "hello from model");
});
