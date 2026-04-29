"use strict";

// Load the native addon built by napi-rs.
const native = require("./index.node");
const fs = require("node:fs");

function wrapCustomWorkerDispatch(dispatch) {
  if (typeof dispatch !== "function") {
    return dispatch;
  }
  return function wrappedDispatch(err, req) {
    if (err != null) {
      throw err;
    }
    return dispatch(req);
  };
}

function withWrappedCustomWorker(opts) {
  if (!opts || typeof opts !== "object" || typeof opts.customWorker !== "function") {
    return opts;
  }
  return {
    ...opts,
    customWorker: wrapCustomWorkerDispatch(opts.customWorker),
  };
}

function assertKnownKeys(object, allowedKeys, label) {
  for (const key of Object.keys(object)) {
    if (!allowedKeys.has(key)) {
      throw new TypeError(`${label} contains unknown key "${key}"`);
    }
  }
}

function normalizeEvalResult(value) {
  if (typeof value === "boolean") {
    return value
      ? { id: "evaluator", status: "passed", passed: true }
      : { id: "evaluator", status: "failed", passed: false, reason: "evaluator returned false" };
  }
  if (!value || typeof value !== "object") {
    throw new TypeError("evaluator must return boolean or EvalResult object");
  }
  const allowedStatuses = new Set(["passed", "failed", "error"]);
  const passed = value.passed ?? value.status === "passed";
  const status = value.status || (passed ? "passed" : "failed");
  if (!allowedStatuses.has(status)) {
    throw new TypeError(`evaluator status must be one of "passed", "failed", "error"; got "${status}"`);
  }
  if (status === "passed" && passed !== true) {
    throw new TypeError('evaluator result is inconsistent: status "passed" requires passed=true');
  }
  if ((status === "failed" || status === "error") && passed !== false) {
    throw new TypeError(`evaluator result is inconsistent: status "${status}" requires passed=false`);
  }
  return {
    id: value.id || "evaluator",
    status,
    passed,
    score: value.score,
    expected: value.expected,
    actual: value.actual,
    reason: value.reason,
    metadata: value.metadata,
  };
}

function buildEvalReport(suiteId, cases) {
  const totalCases = cases.length;
  const passedCases = cases.filter((c) => c.status === "passed").length;
  const failedCases = cases.filter((c) => c.status === "failed").length;
  const errorCases = cases.filter((c) => c.status === "error").length;
  return {
    suiteId,
    status: errorCases ? "error" : failedCases ? "failed" : "passed",
    summary: {
      totalCases,
      passedCases,
      failedCases,
      errorCases,
      passRate: totalCases ? passedCases / totalCases : 0,
    },
    cases,
  };
}

function loadEvalDataset(datasetPath) {
  const records = [];
  const seen = new Set();
  const lines = fs.readFileSync(datasetPath, "utf8").split(/\r?\n/u);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (!line) continue;
    let record;
    try {
      record = JSON.parse(line);
    } catch (err) {
      throw new Error(`failed to parse eval dataset ${datasetPath} line ${index + 1}: ${err.message}`);
    }
    if (!record.id || typeof record.id !== "string") {
      throw new Error(`eval dataset ${datasetPath} line ${index + 1}: id must be a non-empty string`);
    }
    if (seen.has(record.id)) {
      throw new Error(`duplicate eval record id "${record.id}"`);
    }
    seen.add(record.id);
    if (!record.input || typeof record.input !== "object" || Array.isArray(record.input)) {
      throw new Error(`eval record "${record.id}" input must be an object`);
    }
    if (!record.expected_output || typeof record.expected_output !== "object" || Array.isArray(record.expected_output)) {
      throw new Error(`eval record "${record.id}" expected_output must be an object`);
    }
    records.push(record);
  }
  if (records.length === 0) {
    throw new Error(`eval dataset ${datasetPath} must contain at least one record`);
  }
  return records;
}

const clientProto = native.Client && native.Client.prototype;
if (clientProto) {
  const nativeResume = clientProto.resume;
  const nativeRunWorkflow = clientProto.runWorkflow;
  const nativeStreamWorkflow = clientProto.streamWorkflow;

  clientProto.resume = function resume(checkpoint, opts) {
    return nativeResume.call(this, checkpoint, withWrappedCustomWorker(opts));
  };

  clientProto.runWorkflow = function runWorkflow(
    workflowPath,
    workflowInput,
    workflowOptions,
    workflowExecution,
    customWorkerDispatch,
  ) {
    return nativeRunWorkflow.call(
      this,
      workflowPath,
      workflowInput,
      workflowOptions,
      workflowExecution,
      wrapCustomWorkerDispatch(customWorkerDispatch),
    );
  };

  clientProto.streamWorkflow = function streamWorkflow(
    workflowPath,
    workflowInput,
    onEvent,
    workflowOptions,
    workflowExecution,
    customWorkerDispatch,
  ) {
    return nativeStreamWorkflow.call(
      this,
      workflowPath,
      workflowInput,
      onEvent,
      workflowOptions,
      workflowExecution,
      wrapCustomWorkerDispatch(customWorkerDispatch),
    );
  };

  clientProto.runEvalSuite = async function runEvalSuite(request) {
    if (!request || typeof request !== "object") {
      throw new TypeError("runEvalSuite request must be an object");
    }
    assertKnownKeys(
      request,
      new Set([
        "workflowPath",
        "datasetPath",
        "suiteId",
        "execution",
        "workflowOptions",
        "evaluator",
        "customWorkerDispatch",
      ]),
      "runEvalSuite request",
    );
    const { workflowPath, datasetPath, evaluator } = request;
    if (typeof workflowPath !== "string" || !workflowPath.trim()) {
      throw new TypeError("workflowPath is required");
    }
    if (typeof datasetPath !== "string" || !datasetPath.trim()) {
      throw new TypeError("datasetPath is required");
    }
    if (typeof evaluator !== "function") {
      throw new TypeError("evaluator is required");
    }
    const records = loadEvalDataset(datasetPath);
    const cases = [];
    for (const record of records) {
      try {
        const actualOutput = await this.runWorkflow(
          workflowPath,
          record.input,
          request.workflowOptions,
          request.execution,
          request.customWorkerDispatch,
        );
        const evalCase = {
          id: record.id,
          input: record.input,
          expectedOutput: record.expected_output,
          actualOutput,
          record,
        };
        const evaluation = normalizeEvalResult(await evaluator(evalCase));
        cases.push({
          caseId: record.id,
          status: evaluation.status,
          expected: evaluation.expected,
          actual: evaluation.actual,
          evaluations: [evaluation],
          workflowOutput: actualOutput,
          error: evaluation.status === "error"
            ? { code: "evaluator_error", message: evaluation.reason || "evaluator error" }
            : undefined,
        });
      } catch (err) {
        const message = err && err.message ? err.message : String(err);
        cases.push({
          caseId: record.id,
          status: "error",
          evaluations: [{
            id: "evaluator",
            status: "error",
            passed: false,
            reason: message,
          }],
          error: { code: "eval_case_error", message },
        });
      }
    }
    return buildEvalReport(request.suiteId || datasetPath.split(/[\\/]/u).pop().replace(/\.[^.]+$/u, ""), cases);
  };

  clientProto.runWorkflowYaml = function runWorkflowYaml(
    workflowPath,
    workflowInput,
    workflowOptions,
    workflowExecution,
    customWorkerDispatch,
  ) {
    return this.runWorkflow(
      workflowPath,
      workflowInput,
      workflowOptions,
      workflowExecution,
      customWorkerDispatch,
    );
  };

  clientProto.runWorkflowYamlWithEvents = function runWorkflowYamlWithEvents(
    workflowPath,
    workflowInput,
    workflowOptions,
    workflowExecution,
    customWorkerDispatch,
  ) {
    return this.runWorkflow(
      workflowPath,
      workflowInput,
      { ...(workflowOptions ?? {}), include_events: true },
      workflowExecution,
      customWorkerDispatch,
    );
  };

  clientProto.runWorkflowYamlStream = function runWorkflowYamlStream(
    workflowPath,
    workflowInput,
    onEvent,
    workflowOptions,
    workflowExecution,
    customWorkerDispatch,
  ) {
    return this.streamWorkflow(
      workflowPath,
      workflowInput,
      onEvent,
      workflowOptions,
      workflowExecution,
      customWorkerDispatch,
    );
  };

  clientProto.executeWorkflowYaml = function executeWorkflowYaml(request) {
    return this.run(request);
  };

  clientProto.executeWorkflowYamlStream = function executeWorkflowYamlStream(
    request,
    onEvent,
  ) {
    return this.stream(request, onEvent);
  };

  /**
   * Typed workflow run (messages-first request). Delegates to {@link runWorkflow}.
   */
  clientProto.run = function run(request) {
    if (!request || typeof request !== "object") {
      throw new TypeError("request must be an object");
    }
    assertKnownKeys(
      request,
      new Set([
        "workflowPath",
        "workflow_path",
        "messages",
        "healing",
        "workflowStreaming",
        "workflow_streaming",
        "nodeLlmStreaming",
        "node_llm_streaming",
        "splitStreamDeltas",
        "split_stream_deltas",
        "debugStreamParse",
        "debug_stream_parse",
        "extraWorkflowInput",
        "extra_workflow_input",
        "workflowOptions",
        "workflow_options",
        "customWorkerDispatch",
        "custom_worker_dispatch",
      ]),
      "request",
    );
    const workflowPath = request.workflowPath ?? request.workflow_path;
    if (typeof workflowPath !== "string" || !workflowPath.trim()) {
      throw new TypeError("request.workflowPath must be a non-empty string");
    }
    const messages = request.messages;
    if (!Array.isArray(messages)) {
      throw new TypeError("request.messages must be an array");
    }
    const flags = {
      healing: Boolean(request.healing),
      workflowStreaming: Boolean(request.workflowStreaming ?? request.workflow_streaming),
      nodeLlmStreaming: Boolean(request.nodeLlmStreaming ?? request.node_llm_streaming),
    };
    const parsed = native.parseWorkflowYamlExecutionRequest(
      workflowPath,
      messages,
      flags,
      request.extraWorkflowInput ?? request.extra_workflow_input,
      request.workflowOptions ?? request.workflow_options,
    );
    const workflowExecution = {
      healing: parsed.healing,
      workflowStreaming: parsed.workflowStreaming,
      nodeLlmStreaming: parsed.nodeLlmStreaming,
      splitStreamDeltas:
        request.splitStreamDeltas ?? request.split_stream_deltas,
      debugStreamParse:
        request.debugStreamParse ?? request.debug_stream_parse,
    };
    const dispatch =
      request.customWorkerDispatch ?? request.custom_worker_dispatch;
    return this.runWorkflow(
      parsed.workflowPath,
      parsed.workflowInput,
      parsed.workflowOptions,
      workflowExecution,
      dispatch,
    );
  };

  /**
   * Typed workflow stream (messages-first request). Delegates to {@link streamWorkflow}.
   */
  clientProto.stream = function stream(request, onEvent) {
    if (typeof onEvent !== "function") {
      throw new TypeError("onEvent must be a function");
    }
    if (!request || typeof request !== "object") {
      throw new TypeError("request must be an object");
    }
    assertKnownKeys(
      request,
      new Set([
        "workflowPath",
        "workflow_path",
        "messages",
        "healing",
        "workflowStreaming",
        "workflow_streaming",
        "nodeLlmStreaming",
        "node_llm_streaming",
        "splitStreamDeltas",
        "split_stream_deltas",
        "debugStreamParse",
        "debug_stream_parse",
        "extraWorkflowInput",
        "extra_workflow_input",
        "workflowOptions",
        "workflow_options",
        "customWorkerDispatch",
        "custom_worker_dispatch",
      ]),
      "request",
    );
    const workflowPath = request.workflowPath ?? request.workflow_path;
    if (typeof workflowPath !== "string" || !workflowPath.trim()) {
      throw new TypeError("request.workflowPath must be a non-empty string");
    }
    const messages = request.messages;
    if (!Array.isArray(messages)) {
      throw new TypeError("request.messages must be an array");
    }
    const flags = {
      healing: Boolean(request.healing),
      workflowStreaming: Boolean(request.workflowStreaming ?? request.workflow_streaming),
      nodeLlmStreaming: Boolean(request.nodeLlmStreaming ?? request.node_llm_streaming),
    };
    const parsed = native.parseWorkflowYamlExecutionRequest(
      workflowPath,
      messages,
      flags,
      request.extraWorkflowInput ?? request.extra_workflow_input,
      request.workflowOptions ?? request.workflow_options,
    );
    const workflowExecution = {
      healing: parsed.healing,
      workflowStreaming: parsed.workflowStreaming,
      nodeLlmStreaming: parsed.nodeLlmStreaming,
      splitStreamDeltas:
        request.splitStreamDeltas ?? request.split_stream_deltas,
      debugStreamParse:
        request.debugStreamParse ?? request.debug_stream_parse,
    };
    const dispatch =
      request.customWorkerDispatch ?? request.custom_worker_dispatch;
    return this.streamWorkflow(
      parsed.workflowPath,
      parsed.workflowInput,
      onEvent,
      parsed.workflowOptions,
      workflowExecution,
      dispatch,
    );
  };
}

module.exports = native;
module.exports.default = native;
