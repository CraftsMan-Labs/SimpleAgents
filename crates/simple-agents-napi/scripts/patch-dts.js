"use strict";

const fs = require("node:fs");
const path = require("node:path");

const dtsPath = path.join(__dirname, "..", "index.d.ts");
const marker = "\n// --- simple-agents wrapper API additions ---\n";
const generated = fs.readFileSync(dtsPath, "utf8").split(marker)[0].trimEnd();

const additions = `
// --- simple-agents wrapper API additions ---

export interface EvalSuiteRequest {
  suitePath: string
}

export interface EvalSummary {
  totalCases: number
  passedCases: number
  failedCases: number
  errorCases: number
  passRate: number
}

export interface EvalErrorInfo {
  code: string
  message: string
}

export interface EvalCaseResult {
  caseId: string
  status: 'passed' | 'failed' | 'error'
  firstFailedNode?: string
  firstFailedPath?: string
  expected?: unknown
  actual?: unknown
  evaluations?: Array<EvalResult>
  workflowOutput?: Record<string, unknown>
  error?: EvalErrorInfo
}

export interface EvalResult {
  id: string
  kind: 'deterministic' | 'custom'
  status: 'passed' | 'failed' | 'error'
  passed: boolean
  score?: number
  path?: string
  nodeId?: string
  expected?: unknown
  actual?: unknown
  reason?: string
  metadata?: unknown
}

export interface EvalReport {
  suiteId: string
  status: 'passed' | 'failed' | 'error'
  summary: EvalSummary
  cases: Array<EvalCaseResult>
}

export interface WorkflowYamlRunRequest {
  customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown
}

export interface Client {
  run(request: WorkflowYamlRunRequest): Record<string, unknown> | Promise<Record<string, unknown>>
  stream(request: WorkflowYamlRunRequest, onEvent: (eventJson: string) => void): Promise<Record<string, unknown>>
  runWorkflowYaml(workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown>; include_events?: boolean }, workflowExecution?: { healing?: boolean; workflowStreaming?: boolean; nodeLlmStreaming?: boolean; splitStreamDeltas?: boolean }, customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown): Record<string, unknown> | Promise<Record<string, unknown>>
  runWorkflowYamlWithEvents(workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown>; include_events?: boolean }, workflowExecution?: { healing?: boolean; workflowStreaming?: boolean; nodeLlmStreaming?: boolean; splitStreamDeltas?: boolean }, customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown): Record<string, unknown> | Promise<Record<string, unknown>>
  runWorkflowYamlStream(workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, onEvent: (eventJson: string) => void, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown>; include_events?: boolean }, workflowExecution?: { healing?: boolean; workflowStreaming?: boolean; nodeLlmStreaming?: boolean; splitStreamDeltas?: boolean }, customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown): Promise<Record<string, unknown>>
  executeWorkflowYaml(request: WorkflowYamlRunRequest): Record<string, unknown> | Promise<Record<string, unknown>>
  executeWorkflowYamlStream(request: WorkflowYamlRunRequest, onEvent: (eventJson: string) => void): Promise<Record<string, unknown>>
}
`;

fs.writeFileSync(dtsPath, `${generated}\n${additions}`, "utf8");
