use super::*;

struct NodeExecutorContext<'a> {
    runtime: &'a WorkflowRuntime<'a>,
    node: &'a Node,
    node_index: &'a HashMap<&'a str, &'a Node>,
    step: usize,
    scope: &'a mut RuntimeScope,
    cancellation: Option<&'a dyn CancellationSignal>,
    retry_events: &'a mut Vec<WorkflowRetryEvent>,
}

#[async_trait]
trait NodeExecutor: Send + Sync {
    fn supports(&self, kind: &NodeKind) -> bool;

    async fn execute(
        &self,
        ctx: NodeExecutorContext<'_>,
    ) -> Result<NodeExecution, WorkflowRuntimeError>;
}

struct StartNodeExecutor;

#[async_trait]
impl NodeExecutor for StartNodeExecutor {
    fn supports(&self, kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Start { .. })
    }

    async fn execute(
        &self,
        ctx: NodeExecutorContext<'_>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let next = match &ctx.node.kind {
            NodeKind::Start { next } => next,
            _ => unreachable!("start node executor only handles start nodes"),
        };
        Ok(ctx.runtime.execute_start_node(ctx.step, ctx.node, next))
    }
}

struct ConditionNodeExecutor;

#[async_trait]
impl NodeExecutor for ConditionNodeExecutor {
    fn supports(&self, kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::Condition { .. })
    }

    async fn execute(
        &self,
        ctx: NodeExecutorContext<'_>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let (expression, on_true, on_false) = match &ctx.node.kind {
            NodeKind::Condition {
                expression,
                on_true,
                on_false,
            } => (expression, on_true, on_false),
            _ => unreachable!("condition node executor only handles condition nodes"),
        };
        ctx.runtime.execute_condition_node(
            ctx.step,
            ctx.node,
            ConditionNodeSpec {
                expression,
                on_true,
                on_false,
            },
            ctx.scope,
            ctx.cancellation,
        )
    }
}

struct EndNodeExecutor;

#[async_trait]
impl NodeExecutor for EndNodeExecutor {
    fn supports(&self, kind: &NodeKind) -> bool {
        matches!(kind, NodeKind::End)
    }

    async fn execute(
        &self,
        ctx: NodeExecutorContext<'_>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        Ok(NodeExecution {
            step: ctx.step,
            node_id: ctx.node.id.clone(),
            data: NodeExecutionData::End,
        })
    }
}

struct GeneralNodeExecutor;

#[async_trait]
impl NodeExecutor for GeneralNodeExecutor {
    fn supports(&self, _kind: &NodeKind) -> bool {
        true
    }

    async fn execute(
        &self,
        ctx: NodeExecutorContext<'_>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        match &ctx.node.kind {
            NodeKind::Llm {
                model,
                prompt,
                next,
            } => {
                ctx.runtime
                    .execute_llm_node(
                        ctx.step,
                        ctx.node,
                        LlmNodeSpec {
                            model,
                            prompt,
                            next,
                        },
                        ctx.scope,
                        ctx.cancellation,
                        ctx.retry_events,
                    )
                    .await
            }
            NodeKind::Tool { tool, input, next } => {
                ctx.runtime
                    .execute_tool_node(
                        ctx.step,
                        ctx.node,
                        ToolNodeSpec { tool, input, next },
                        ctx.scope,
                        ctx.cancellation,
                        ctx.retry_events,
                    )
                    .await
            }
            NodeKind::Debounce {
                key_path,
                window_steps,
                next,
                on_suppressed,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: ctx.node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let suppressed = ctx
                    .scope
                    .debounce(&ctx.node.id, &key, ctx.step, *window_steps);
                let chosen_next = if suppressed {
                    on_suppressed.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"key": key.clone(), "suppressed": suppressed}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Debounce {
                        key,
                        suppressed,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Throttle {
                key_path,
                window_steps,
                next,
                on_throttled,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: ctx.node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let throttled = ctx
                    .scope
                    .throttle(&ctx.node.id, &key, ctx.step, *window_steps);
                let chosen_next = if throttled {
                    on_throttled.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"key": key.clone(), "throttled": throttled}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Throttle {
                        key,
                        throttled,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::RetryCompensate {
                tool,
                input,
                max_retries,
                compensate_tool,
                compensate_input,
                next,
                on_compensated,
            } => {
                let executor = ctx.runtime.tool_executor.ok_or_else(|| {
                    WorkflowRuntimeError::MissingToolExecutor {
                        node_id: ctx.node.id.clone(),
                    }
                })?;
                let scoped_input =
                    ctx.scope
                        .scoped_input(ScopeCapability::ToolRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: ctx.node.id.clone(),
                            source,
                        })?;
                let total_attempts = max_retries.saturating_add(1);
                let mut last_error = None;
                let mut output = Value::Null;
                let mut compensated = false;

                for attempt in 1..=total_attempts {
                    check_cancelled(ctx.cancellation)?;
                    match executor
                        .execute_tool(ToolExecutionInput {
                            node_id: ctx.node.id.clone(),
                            tool: tool.clone(),
                            input: input.clone(),
                            scoped_input: scoped_input.clone(),
                        })
                        .await
                    {
                        Ok(value) => {
                            output = json!({"status": "ok", "attempt": attempt, "value": value});
                            break;
                        }
                        Err(error) => {
                            last_error = Some(error.clone());
                            if attempt < total_attempts {
                                ctx.retry_events.push(WorkflowRetryEvent {
                                    step: ctx.step,
                                    node_id: ctx.node.id.clone(),
                                    operation: "retry_compensate".to_string(),
                                    failed_attempt: attempt,
                                    reason: error.to_string(),
                                });
                            }
                        }
                    }
                }

                if output.is_null() {
                    compensated = true;
                    let compensation = executor
                        .execute_tool(ToolExecutionInput {
                            node_id: ctx.node.id.clone(),
                            tool: compensate_tool.clone(),
                            input: compensate_input.clone(),
                            scoped_input,
                        })
                        .await
                        .map_err(|compensation_error| {
                            WorkflowRuntimeError::RetryCompensateFailed {
                                node_id: ctx.node.id.clone(),
                                attempts: total_attempts,
                                compensation_error,
                            }
                        })?;
                    output = json!({
                        "status": "compensated",
                        "attempts": total_attempts,
                        "last_error": last_error.map(|error| error.to_string()).unwrap_or_default(),
                        "compensation": compensation
                    });
                }

                let chosen_next = if compensated {
                    on_compensated.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                ctx.scope
                    .record_node_output(&ctx.node.id, output.clone(), ScopeCapability::MapWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::RetryCompensate {
                        tool: tool.clone(),
                        attempts: total_attempts,
                        compensated,
                        output,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::HumanInTheLoop {
                decision_path,
                response_path,
                on_approve,
                on_reject,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let decision_value =
                    resolve_path(&scoped_input, decision_path).ok_or_else(|| {
                        WorkflowRuntimeError::MissingPath {
                            node_id: ctx.node.id.clone(),
                            path: decision_path.clone(),
                        }
                    })?;
                let approved = evaluate_human_decision(decision_value).ok_or_else(|| {
                    WorkflowRuntimeError::InvalidHumanDecision {
                        node_id: ctx.node.id.clone(),
                        path: decision_path.clone(),
                        value: decision_value.to_string(),
                    }
                })?;
                let response = if let Some(path) = response_path {
                    resolve_path(&scoped_input, path).cloned().ok_or_else(|| {
                        WorkflowRuntimeError::MissingPath {
                            node_id: ctx.node.id.clone(),
                            path: path.clone(),
                        }
                    })?
                } else {
                    Value::Null
                };
                let chosen_next = if approved {
                    on_approve.clone()
                } else {
                    on_reject.clone()
                };

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"approved": approved, "response": response.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::HumanInTheLoop {
                        approved,
                        response,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::CacheWrite {
                key_path,
                value_path,
                next,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: ctx.node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let value = resolve_path(&scoped_input, value_path)
                    .cloned()
                    .ok_or_else(|| WorkflowRuntimeError::MissingPath {
                        node_id: ctx.node.id.clone(),
                        path: value_path.clone(),
                    })?;

                ctx.scope.put_cache(&key, value.clone());
                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"key": key.clone(), "value": value.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::CacheWrite {
                        key,
                        value,
                        next: next.clone(),
                    },
                })
            }
            NodeKind::CacheRead {
                key_path,
                next,
                on_miss,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: ctx.node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let value = ctx.scope.cache_value(&key).cloned().unwrap_or(Value::Null);
                let hit = !value.is_null();
                let chosen_next = if hit {
                    next.clone()
                } else {
                    on_miss.clone().unwrap_or_else(|| next.clone())
                };

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"key": key.clone(), "hit": hit, "value": value.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::CacheRead {
                        key,
                        hit,
                        value,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::EventTrigger {
                event,
                event_path,
                next,
                on_mismatch,
            } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let actual = resolve_path(&scoped_input, event_path)
                    .and_then(Value::as_str)
                    .ok_or_else(|| WorkflowRuntimeError::InvalidEventValue {
                        node_id: ctx.node.id.clone(),
                        path: event_path.clone(),
                    })?;
                let matched = actual == event;
                let chosen_next = if matched {
                    next.clone()
                } else {
                    on_mismatch.clone().unwrap_or_else(|| next.clone())
                };

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"event": event.clone(), "matched": matched, "actual": actual}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::EventTrigger {
                        event: event.clone(),
                        matched,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Router { routes, default } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                enforce_expression_scope_budget(
                    &ctx.node.id,
                    &scoped_input,
                    ctx.runtime
                        .options
                        .security_limits
                        .max_expression_scope_bytes,
                )?;
                let mut selected = default.clone();
                for route in routes {
                    let matched = expressions::evaluate_bool(&route.when, &scoped_input).map_err(
                        |reason| WorkflowRuntimeError::InvalidRouterExpression {
                            node_id: ctx.node.id.clone(),
                            expression: route.when.clone(),
                            reason: reason.to_string(),
                        },
                    )?;
                    if matched {
                        selected = route.next.clone();
                        break;
                    }
                }

                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        json!({"selected": selected.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Router {
                        selected: selected.clone(),
                        next: selected,
                    },
                })
            }
            NodeKind::Transform { expression, next } => {
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                let output =
                    evaluate_transform_expression(expression, &scoped_input).map_err(|reason| {
                        WorkflowRuntimeError::InvalidTransformExpression {
                            node_id: ctx.node.id.clone(),
                            expression: expression.clone(),
                            reason,
                        }
                    })?;

                ctx.scope
                    .record_node_output(&ctx.node.id, output.clone(), ScopeCapability::MapWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Transform {
                        expression: expression.clone(),
                        output,
                        next: next.clone(),
                    },
                })
            }
            NodeKind::Loop {
                condition,
                body,
                next,
                max_iterations,
            } => {
                check_cancelled(ctx.cancellation)?;
                let scoped_input = ctx
                    .scope
                    .scoped_input(ScopeCapability::ConditionRead)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;
                enforce_expression_scope_budget(
                    &ctx.node.id,
                    &scoped_input,
                    ctx.runtime
                        .options
                        .security_limits
                        .max_expression_scope_bytes,
                )?;
                let evaluated =
                    expressions::evaluate_bool(condition, &scoped_input).map_err(|reason| {
                        WorkflowRuntimeError::InvalidLoopCondition {
                            node_id: ctx.node.id.clone(),
                            expression: condition.clone(),
                            reason: reason.to_string(),
                        }
                    })?;

                let (iteration, chosen_next) = if evaluated {
                    let iteration = ctx.scope.loop_iteration(&ctx.node.id).saturating_add(1);
                    if let Some(limit) = max_iterations {
                        if iteration > *limit {
                            return Err(WorkflowRuntimeError::LoopIterationLimitExceeded {
                                node_id: ctx.node.id.clone(),
                                max_iterations: *limit,
                            });
                        }
                    }
                    ctx.scope.set_loop_iteration(&ctx.node.id, iteration);
                    (iteration, body.clone())
                } else {
                    ctx.scope.clear_loop_iteration(&ctx.node.id);
                    (0, next.clone())
                };

                ctx.scope
                    .record_condition_output(
                        &ctx.node.id,
                        evaluated,
                        ScopeCapability::ConditionWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Loop {
                        condition: condition.clone(),
                        evaluated,
                        iteration,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Parallel {
                branches,
                next,
                max_in_flight,
            } => {
                ctx.runtime
                    .execute_parallel_node(
                        ctx.step,
                        ctx.node,
                        ParallelNodeSpec {
                            node_index: ctx.node_index,
                            branches,
                            next,
                            max_in_flight: *max_in_flight,
                        },
                        ctx.scope,
                        ctx.cancellation,
                        ctx.retry_events,
                    )
                    .await
            }
            NodeKind::Merge {
                sources,
                policy,
                quorum,
                next,
            } => ctx.runtime.execute_merge_node(
                ctx.step,
                ctx.node,
                MergeNodeSpec {
                    sources,
                    policy,
                    quorum: *quorum,
                    next,
                },
                ctx.scope,
            ),
            NodeKind::Map {
                tool,
                items_path,
                next,
                max_in_flight,
            } => {
                ctx.runtime
                    .execute_map_node(
                        ctx.step,
                        ctx.node,
                        MapNodeSpec {
                            tool,
                            items_path,
                            next,
                            max_in_flight: *max_in_flight,
                        },
                        ctx.scope,
                        ctx.cancellation,
                        ctx.retry_events,
                    )
                    .await
            }
            NodeKind::Reduce {
                source,
                operation,
                next,
            } => ctx
                .runtime
                .execute_reduce_node(ctx.step, ctx.node, source, operation, next, ctx.scope),
            NodeKind::Subgraph { graph, next } => {
                check_cancelled(ctx.cancellation)?;
                let next_node =
                    next.clone()
                        .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                            node_id: ctx.node.id.clone(),
                        })?;
                let subgraph = ctx
                    .runtime
                    .options
                    .subgraph_registry
                    .get(graph)
                    .ok_or_else(|| WorkflowRuntimeError::SubgraphNotFound {
                        node_id: ctx.node.id.clone(),
                        graph: graph.clone(),
                    })?;

                let subgraph_runtime = WorkflowRuntime::new(
                    subgraph.clone(),
                    ctx.runtime.llm_executor,
                    ctx.runtime.tool_executor,
                    WorkflowRuntimeOptions {
                        replay_mode: WorkflowReplayMode::Disabled,
                        enable_trace_recording: false,
                        ..ctx.runtime.options.clone()
                    },
                );
                let subgraph_result = Box::pin(
                    subgraph_runtime.execute(
                        ctx.scope
                            .scoped_input(ScopeCapability::ConditionRead)
                            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                                node_id: ctx.node.id.clone(),
                                source,
                            })?,
                        ctx.cancellation,
                    ),
                )
                .await?;

                let subgraph_output = json!({
                    "terminal_node_id": subgraph_result.terminal_node_id,
                    "node_outputs": subgraph_result.node_outputs,
                });
                ctx.scope
                    .record_node_output(
                        &ctx.node.id,
                        subgraph_output.clone(),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: ctx.node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step: ctx.step,
                    node_id: ctx.node.id.clone(),
                    data: NodeExecutionData::Subgraph {
                        graph: graph.clone(),
                        terminal_node_id: subgraph_result.terminal_node_id,
                        output: subgraph_output,
                        next: next_node,
                    },
                })
            }
            NodeKind::Batch { items_path, next } => ctx
                .runtime
                .execute_batch_node(ctx.step, ctx.node, items_path, next, ctx.scope),
            NodeKind::Filter {
                items_path,
                expression,
                next,
            } => ctx
                .runtime
                .execute_filter_node(ctx.step, ctx.node, items_path, expression, next, ctx.scope),
            NodeKind::Start { .. } | NodeKind::Condition { .. } | NodeKind::End => {
                unreachable!("specific strategy executors should handle start/condition/end")
            }
        }
    }
}

fn strategy_executors() -> [&'static dyn NodeExecutor; 4] {
    static START: StartNodeExecutor = StartNodeExecutor;
    static CONDITION: ConditionNodeExecutor = ConditionNodeExecutor;
    static END: EndNodeExecutor = EndNodeExecutor;
    static GENERAL: GeneralNodeExecutor = GeneralNodeExecutor;
    [&START, &CONDITION, &END, &GENERAL]
}

async fn execute_node_via_strategy(
    ctx: NodeExecutorContext<'_>,
) -> Result<Option<NodeExecution>, WorkflowRuntimeError> {
    for executor in strategy_executors() {
        if executor.supports(&ctx.node.kind) {
            return executor.execute(ctx).await.map(Some);
        }
    }
    Ok(None)
}

pub(super) async fn execute_from_node(
    runtime: &WorkflowRuntime<'_>,
    workflow: WorkflowDefinition,
    mut scope: RuntimeScope,
    start_node_id: String,
    starting_step: usize,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<WorkflowRunResult, WorkflowRuntimeError> {
    let node_index = build_node_index(&workflow);

    if matches!(
        runtime.options.replay_mode,
        WorkflowReplayMode::ValidateRecordedTrace
    ) && !runtime.options.enable_trace_recording
    {
        return Err(WorkflowRuntimeError::ReplayRequiresTraceRecording);
    }

    let trace_recorder = runtime.options.enable_trace_recording.then(|| {
        TraceRecorder::new(WorkflowTraceMetadata {
            trace_id: format!("{}-{}-trace", workflow.name, workflow.version),
            workflow_name: workflow.name.clone(),
            workflow_version: workflow.version.clone(),
            started_at_unix_ms: 0,
            finished_at_unix_ms: None,
        })
    });
    let mut trace_clock = 0u64;
    let mut events = Vec::new();
    let mut retry_events = Vec::new();
    let mut node_executions = Vec::new();
    let mut current_id = start_node_id;

    for step in starting_step..runtime.options.max_steps {
        check_cancelled(cancellation)?;
        let node = node_index.get(current_id.as_str()).ok_or_else(|| {
            WorkflowRuntimeError::NodeNotFound {
                node_id: current_id.clone(),
            }
        })?;

        events.push(WorkflowEvent {
            step,
            node_id: current_id.clone(),
            kind: WorkflowEventKind::NodeStarted,
        });

        if let Some(recorder) = &trace_recorder {
            recorder.record_node_enter(next_trace_timestamp(&mut trace_clock), &current_id)?;
        }

        let execution_result = runtime
            .execute_node(
                node,
                &node_index,
                step,
                &mut scope,
                cancellation,
                &mut retry_events,
            )
            .await;
        let execution = match execution_result {
            Ok(execution) => execution,
            Err(error) => {
                if let Some(recorder) = &trace_recorder {
                    recorder.record_node_error(
                        next_trace_timestamp(&mut trace_clock),
                        &current_id,
                        error.to_string(),
                    )?;
                    recorder.record_terminal(
                        next_trace_timestamp(&mut trace_clock),
                        TraceTerminalStatus::Failed,
                    )?;
                    let _ = recorder.finalize(next_trace_timestamp(&mut trace_clock))?;
                }
                events.push(WorkflowEvent {
                    step,
                    node_id: current_id,
                    kind: WorkflowEventKind::NodeFailed {
                        message: error.to_string(),
                    },
                });
                return Err(error);
            }
        };

        events.push(WorkflowEvent {
            step,
            node_id: execution.node_id.clone(),
            kind: WorkflowEventKind::NodeCompleted {
                data: execution.data.clone(),
            },
        });

        if let Some(recorder) = &trace_recorder {
            recorder
                .record_node_exit(next_trace_timestamp(&mut trace_clock), &execution.node_id)?;
        }

        let is_terminal = matches!(execution.data, NodeExecutionData::End);
        let next_node = next_node_id(&execution.data);
        let executed_node_id = execution.node_id.clone();
        node_executions.push(execution);

        if is_terminal {
            let (trace, replay_report) = if let Some(recorder) = &trace_recorder {
                recorder.record_terminal(
                    next_trace_timestamp(&mut trace_clock),
                    TraceTerminalStatus::Completed,
                )?;
                let finalized_trace = recorder.finalize(next_trace_timestamp(&mut trace_clock))?;
                let replay_report = match runtime.options.replay_mode {
                    WorkflowReplayMode::Disabled => None,
                    WorkflowReplayMode::ValidateRecordedTrace => {
                        Some(replay_trace(&finalized_trace)?)
                    }
                };
                (Some(finalized_trace), replay_report)
            } else {
                (None, None)
            };

            return Ok(WorkflowRunResult {
                workflow_name: workflow.name,
                terminal_node_id: executed_node_id,
                node_executions,
                events,
                retry_events,
                node_outputs: scope.into_node_outputs_btree(),
                trace,
                replay_report,
            });
        }

        current_id = next_node.ok_or(WorkflowRuntimeError::MissingNextTransition {
            node_id: executed_node_id,
        })?;
    }

    Err(WorkflowRuntimeError::StepLimitExceeded {
        max_steps: runtime.options.max_steps,
    })
}

pub(super) async fn execute_node(
    runtime: &WorkflowRuntime<'_>,
    node: &Node,
    node_index: &HashMap<&str, &Node>,
    step: usize,
    scope: &mut RuntimeScope,
    cancellation: Option<&dyn CancellationSignal>,
    retry_events: &mut Vec<WorkflowRetryEvent>,
) -> Result<NodeExecution, WorkflowRuntimeError> {
    if let Some(execution) = execute_node_via_strategy(NodeExecutorContext {
        runtime,
        node,
        node_index,
        step,
        scope,
        cancellation,
        retry_events,
    })
    .await?
    {
        return Ok(execution);
    }

    unreachable!("no strategy executor registered for node kind")
}
