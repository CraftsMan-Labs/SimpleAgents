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
  const nativeRun = clientProto.run;
  const nativeStream = clientProto.stream;
  const nativeResume = clientProto.resume;
  const nativeRunWorkflow = clientProto.runWorkflow;
  const nativeStreamWorkflow = clientProto.streamWorkflow;

  clientProto.run = function run(workflowPath, messages, opts) {
    return nativeRun.call(this, workflowPath, messages, withWrappedCustomWorker(opts));
  };

  clientProto.stream = function stream(workflowPath, messages, onEvent, opts) {
    return nativeStream.call(this, workflowPath, messages, onEvent, withWrappedCustomWorker(opts));
  };

  clientProto.resume = function resume(checkpoint, opts) {
    return nativeResume.call(this, checkpoint, withWrappedCustomWorker(opts));
  };

  clientProto.runWorkflow = function runWorkflow(
    workflowPath,
    workflowInput,
    workflowOptions,
    customWorkerDispatch,
  ) {
    return nativeRunWorkflow.call(
      this,
      workflowPath,
      workflowInput,
      workflowOptions,
      wrapCustomWorkerDispatch(customWorkerDispatch),
    );
  };

  clientProto.streamWorkflow = function streamWorkflow(
    workflowPath,
    workflowInput,
    onEvent,
    workflowOptions,
    customWorkerDispatch,
  ) {
    return nativeStreamWorkflow.call(
      this,
      workflowPath,
      workflowInput,
      onEvent,
      workflowOptions,
      wrapCustomWorkerDispatch(customWorkerDispatch),
    );
  };
}

module.exports = native;
module.exports.default = native;
