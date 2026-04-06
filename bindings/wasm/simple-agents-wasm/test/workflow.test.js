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
        slugify: async ({ input }) => String(input).toLowerCase().replace(/\s+/g, "-")
      }
    }
  );

  assert.equal(result.status, "ok");
  assert.equal(result.context.make_slug, "yamslam-playground");
  assert.equal(result.terminal_output, "ok=yamslam-playground");
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
  assert.equal(result.terminal_output, "hello from model");
});

test("runWorkflowYamlString executes graph workflow with input.messages + switch", async () => {
  let requestBody;
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async (_url, init) => {
      requestBody = JSON.parse(init.body);
      return makeJsonResponse('{"state":"ready","reason":"enough context"}');
    }
  });

  const yaml = `
id: email-chat-draft-or-clarify
entry_node: detect
nodes:
  - id: detect
    node_type:
      llm_call:
        model: gpt-4o-mini
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
          reason:
            type: string
        required: [state, reason]
        additionalProperties: false
      prompt: "Classify state"

  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.detect.output.state == "ready"'
            target: done
        default: ask

  - id: ask
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: ask_for_context

  - id: done
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: ready

edges:
  - from: detect
    to: route
`;

  const result = await client.runWorkflowYamlString(yaml, {
    messages: [{ role: "user", content: "Help me draft an email" }]
  }, {
    functions: {
      GetRagData: ({ payload }) => ({
        topic: payload.topic,
        decision: "handled"
      })
    }
  });

  assert.equal(requestBody.model, "gpt-4o-mini");
  assert.equal(requestBody.messages.length, 2);
  assert.equal(requestBody.messages[0].content, "Help me draft an email");
  assert.equal(requestBody.messages[1].content, "Classify state");
  assert.equal(result.status, "ok");
  assert.equal(result.context.nodes.detect.output.state, "ready");
  assert.equal(result.context.nodes.done.output.topic, "ready");
});

test("runWorkflowYamlString graph llm_call enforces output_schema", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse('{"state":"ready"}')
  });

  const yaml = `
entry_node: detect
nodes:
  - id: detect
    node_type:
      llm_call:
        model: gpt-4o-mini
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
          reason:
            type: string
        required: [state, reason]
        additionalProperties: false
      prompt: "Classify state"
`;

  await assert.rejects(
    async () => {
      await client.runWorkflowYamlString(yaml, { messages: [] });
    },
    /output failed schema validation/
  );
});

test("runWorkflowYamlString graph custom_worker interpolates payload from prior nodes", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse('{"company_name":"Google","reason":"seller found"}')
  });

  const yaml = `
entry_node: extract_company
nodes:
  - id: extract_company
    node_type:
      llm_call:
        model: gpt-4o-mini
    config:
      output_schema:
        type: object
        properties:
          company_name:
            type: string
          reason:
            type: string
        required: [company_name, reason]
        additionalProperties: false
      prompt: "Extract company"

  - id: lookup_company
    node_type:
      custom_worker:
        handler: get_seller_name
    config:
      payload:
        company_name: "{{ nodes.extract_company.output.company_name }}"

edges:
  - from: extract_company
    to: lookup_company
`;

  const result = await client.runWorkflowYamlString(
    yaml,
    { messages: [] },
    {
      functions: {
        get_seller_name: ({ payload }) => ({
          company_name: payload.company_name,
          stakeholder_name: payload.company_name === "Google" ? "Sundar Pichai" : "unknown"
        })
      }
    }
  );

  assert.equal(result.status, "ok");
  assert.equal(result.context.nodes.extract_company.output.company_name, "Google");
  assert.equal(result.context.nodes.lookup_company.output.company_name, "Google");
  assert.equal(result.context.nodes.lookup_company.output.stakeholder_name, "Sundar Pichai");
});

test("runWorkflowYamlString graph custom_worker requires registered handler", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse('{"state":"ready","reason":"ok"}')
  });

  const yaml = `
entry_node: detect
nodes:
  - id: detect
    node_type:
      llm_call:
        model: gpt-4o-mini
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
          reason:
            type: string
        required: [state, reason]
        additionalProperties: false
      prompt: "Classify state"
  - id: next
    node_type:
      custom_worker:
        handler: GetRagData
edges:
  - from: detect
    to: next
`;

  await assert.rejects(
    async () => {
      await client.runWorkflowYamlString(yaml, { messages: [] });
    },
    /requires workflowOptions\.functions\['GetRagData'\]/
  );
});

test("runWorkflowYamlString graph custom_worker supports handler_file namespaced lookup", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse('{"state":"ready","reason":"ok"}')
  });

  const yaml = `
entry_node: detect
nodes:
  - id: detect
    node_type:
      llm_call:
        model: gpt-4o-mini
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
          reason:
            type: string
        required: [state, reason]
      prompt: "Classify state"
  - id: next
    node_type:
      custom_worker:
        handler_file: handlers.py
        handler: get_rag_data
edges:
  - from: detect
    to: next
`;

  const result = await client.runWorkflowYamlString(yaml, { messages: [] }, {
    functions: {
      "handlers.py#get_rag_data": ({ payload }) => ({ topic: payload?.topic ?? "default" })
    }
  });

  assert.equal(result.status, "ok");
  assert.equal(result.context.nodes.next.output.topic, "default");
});

test("runWorkflowYamlString normalizes legacy rust workflow result shape", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse("unused")
  });

  client.ensureBackend = async () => ({
    runWorkflowYamlString: async () => ({
      status: "ok",
      context: {
        input: { email_text: "Need help" },
        nodes: {
          classify: { output: { state: "ready" } }
        }
      },
      output: { state: "ready" },
      events: [{ stepId: "classify", status: "completed" }]
    })
  });

  const result = await client.runWorkflowYamlString("steps: []", {});
  assert.equal(result.workflow_id, "wasm_workflow");
  assert.equal(result.outputs.classify.output.state, "ready");
  assert.equal(result.terminal_node, "classify");
  assert.equal(result.terminal_output.state, "ready");
});

test("client methods fail hard when Rust backend initialization fails", async () => {
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: async () => makeJsonResponse("unused")
  });

  client.ensureBackend = async () => {
    throw new Error("Rust backend unavailable");
  };

  await assert.rejects(
    async () => {
      await client.complete("gpt-4o-mini", "hello");
    },
    /Rust backend unavailable/
  );
});
