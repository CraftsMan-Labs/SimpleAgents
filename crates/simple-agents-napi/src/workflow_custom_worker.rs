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
    tsfn: ThreadsafeFunction<CustomWorkerJob, ErrorStrategy::CalleeHandled>,
}

/// Build an executor that dispatches each `custom_worker` invocation to a JS
/// callback on the Node main thread and awaits its return value.
///
/// The JS `dispatch` function receives a single argument
/// `{ handler, handlerFile?, payload, context }` and must return a
/// JSON-serializable value (synchronous return or a Promise are both accepted).
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
