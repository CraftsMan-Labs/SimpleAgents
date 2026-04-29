"use strict";

// Load the native addon built by napi-rs.
const native = require("./index.node");

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

function camelizeEvalResult(report) {
  if (!report || typeof report !== "object") {
    return report;
  }
  const summary = report.summary && typeof report.summary === "object"
    ? {
        totalCases: report.summary.total_cases,
        passedCases: report.summary.passed_cases,
        failedCases: report.summary.failed_cases,
        errorCases: report.summary.error_cases,
        passRate: report.summary.pass_rate,
      }
    : report.summary;
  const cases = Array.isArray(report.cases)
    ? report.cases.map((caseResult) => ({
        caseId: caseResult.case_id,
        status: caseResult.status,
        firstFailedNode: caseResult.first_failed_node,
        firstFailedPath: caseResult.first_failed_path,
        expected: caseResult.expected,
        actual: caseResult.actual,
        evaluations: Array.isArray(caseResult.evaluations)
          ? caseResult.evaluations.map((evaluation) => ({
              id: evaluation.id,
              kind: evaluation.kind,
              status: evaluation.status,
              passed: evaluation.passed,
              score: evaluation.score,
              path: evaluation.path,
              nodeId: evaluation.node_id,
              expected: evaluation.expected,
              actual: evaluation.actual,
              reason: evaluation.reason,
              metadata: evaluation.metadata,
            }))
          : caseResult.evaluations,
        workflowOutput: caseResult.workflow_output,
        error: caseResult.error,
      }))
    : report.cases;
  return {
    suiteId: report.suite_id,
    status: report.status,
    summary,
    cases,
  };
}

const clientProto = native.Client && native.Client.prototype;
if (clientProto) {
  const nativeResume = clientProto.resume;
  const nativeRunWorkflow = clientProto.runWorkflow;
  const nativeStreamWorkflow = clientProto.streamWorkflow;
  const nativeRunEvalSuite = clientProto.runEvalSuite;

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

  clientProto.runEvalSuite = function runEvalSuite(
    request,
    customWorkerDispatch,
  ) {
    return Promise.resolve(
      nativeRunEvalSuite.call(
        this,
        request,
        wrapCustomWorkerDispatch(customWorkerDispatch),
      ),
    ).then(camelizeEvalResult);
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
