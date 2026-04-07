//! Node.js callback executor for YAML `custom_worker` nodes (parity with Python `handlers.py`).

use async_trait::async_trait;
use napi::bindgen_prelude::JsFunction;
use napi::threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction};
use napi::JsUnknown;
use serde_json::{json, Value};
use simple_agents_workflow::yaml_runner::YamlWorkflowCustomWorkerExecutor;
use std::result::Result as StdResult;
use std::sync::Arc;

struct CustomWorkerJob {
    handler: String,
    handler_file: Option<String>,
    payload: Value,
    context: Value,
}

struct NodeCustomWorkerExecutor {
    /// Internal callback shape is `(err, req)` due to `CalleeHandled`.
    /// The public Node API wraps user handlers back to `(req) => unknown`.
    tsfn: ThreadsafeFunction<CustomWorkerJob, ErrorStrategy::CalleeHandled>,
}

/// Build an executor that dispatches each `custom_worker` invocation to a JS
/// callback on the Node main thread and awaits its return value.
///
/// Internal callback shape is `(err, req)` where `req` is
/// `{ handler, handlerFile?, payload, context }` and success passes `err = null`.
/// The Node wrapper adapts user handlers to the public `(req) => unknown` API.
/// The handler result must be JSON-serializable (sync return or Promise).
pub(crate) fn build_executor(
    dispatch: &JsFunction,
) -> napi::Result<Arc<dyn YamlWorkflowCustomWorkerExecutor>> {
    let tsfn: ThreadsafeFunction<CustomWorkerJob, ErrorStrategy::CalleeHandled> = dispatch
        .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<CustomWorkerJob>| {
            let env = ctx.env;
            let job = ctx.value;
            let req = json!({
                "handler": job.handler,
                "handlerFile": job.handler_file,
                "payload": job.payload,
                "context": job.context,
            });
            let js_val: JsUnknown = env.to_js_value(&req)?;
            Ok(vec![js_val])
        })?;
    Ok(Arc::new(NodeCustomWorkerExecutor { tsfn }))
}

#[async_trait]
impl YamlWorkflowCustomWorkerExecutor for NodeCustomWorkerExecutor {
    async fn execute(
        &self,
        handler: &str,
        handler_file: Option<&str>,
        payload: &Value,
        context: &Value,
    ) -> StdResult<Value, String> {
        let job = CustomWorkerJob {
            handler: handler.to_string(),
            handler_file: handler_file.map(String::from),
            payload: payload.clone(),
            context: context.clone(),
        };
        self.tsfn
            .call_async::<Value>(Ok(job))
            .await
            .map_err(|e| e.to_string())
    }
}
