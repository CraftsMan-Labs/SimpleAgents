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

  /**
   * Typed workflow run (messages-first request). Delegates to {@link runWorkflow}.
   */
  clientProto.run = function run(request) {
    if (!request || typeof request !== "object") {
      throw new TypeError("request must be an object");
    }
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
      workflowStreaming: Boolean(request.workflowStreaming),
      nodeLlmStreaming: Boolean(request.nodeLlmStreaming),
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
      workflowStreaming: Boolean(request.workflowStreaming),
      nodeLlmStreaming: Boolean(request.nodeLlmStreaming),
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
