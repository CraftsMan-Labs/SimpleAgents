const fs = require('node:fs')
const path = require('node:path')
const readline = require('node:readline/promises')
const { stdin, stdout } = require('node:process')

function parseArgs(argv) {
  const options = {
    workflow: 'workflow_email/email-chat-draft-or-clarify.yaml',
    includeEvents: false,
    maxTurns: 8,
    stream: false,
    showThinking: false,
    traceDir: 'examples/workflow_email/traces',
    showStepJson: false,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--workflow' && argv[i + 1]) {
      options.workflow = argv[i + 1]
      i += 1
      continue
    }
    if (arg === '--max-turns' && argv[i + 1]) {
      const parsed = Number.parseInt(argv[i + 1], 10)
      if (Number.isFinite(parsed) && parsed > 0) {
        options.maxTurns = parsed
      }
      i += 1
      continue
    }
    if (arg === '--include-events') {
      options.includeEvents = true
      continue
    }
    if (arg === '--stream') {
      options.stream = true
      continue
    }
    if (arg === '--show-thinking') {
      options.showThinking = true
      continue
    }
    if (arg === '--trace-dir' && argv[i + 1]) {
      options.traceDir = argv[i + 1]
      i += 1
      continue
    }
    if (arg === '--show-step-json') {
      options.showStepJson = true
      continue
    }
  }

  return options
}

function loadDotEnv(filePath) {
  if (!fs.existsSync(filePath)) return
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed === '' || trimmed.startsWith('#')) continue
    const splitIndex = trimmed.indexOf('=')
    if (splitIndex <= 0) continue
    const key = trimmed.slice(0, splitIndex).trim()
    const value = trimmed.slice(splitIndex + 1).trim()
    if (process.env[key] === undefined) {
      process.env[key] = value
    }
  }
}

function loadConfig() {
  loadDotEnv(path.resolve(__dirname, '../../.env'))
  loadDotEnv(path.resolve(process.cwd(), '.env'))

  const provider = process.env.WORKFLOW_PROVIDER || 'openai'
  const apiBase = process.env.WORKFLOW_API_BASE || process.env.CUSTOM_API_BASE || ''
  const apiKey = process.env.WORKFLOW_API_KEY || process.env.CUSTOM_API_KEY || ''

  if (apiBase === '' || apiKey === '') {
    throw new Error('Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY).')
  }

  return { provider, apiBase, apiKey }
}

function mapProviderEnv(provider, apiKey, apiBase) {
  if (provider === 'openai') {
    process.env.OPENAI_API_KEY = apiKey
    if (apiBase !== '') process.env.OPENAI_API_BASE = apiBase
    return
  }
  if (provider === 'anthropic') {
    process.env.ANTHROPIC_API_KEY = apiKey
    return
  }
  if (provider === 'openrouter') {
    process.env.OPENROUTER_API_KEY = apiKey
    if (apiBase !== '') process.env.OPENROUTER_API_BASE = apiBase
    return
  }
  throw new Error(`Unsupported WORKFLOW_PROVIDER: ${provider}`)
}

function resolveWorkflowPath(workflow) {
  const direct = path.resolve(process.cwd(), workflow)
  if (fs.existsSync(direct)) return direct

  const fromExamplesDir = path.resolve(__dirname, '../../', workflow)
  if (fs.existsSync(fromExamplesDir)) return fromExamplesDir

  const repoRelative = path.resolve(__dirname, '../../../', workflow)
  if (fs.existsSync(repoRelative)) return repoRelative

  if (workflow.startsWith('examples/')) {
    const trimmed = workflow.slice('examples/'.length)
    const trimmedPath = path.resolve(__dirname, '../../', trimmed)
    if (fs.existsSync(trimmedPath)) return trimmedPath
  }

  throw new Error(`workflow file not found: ${workflow}`)
}

function initialMessages() {
  return [
    {
      role: 'system',
      content:
        'You are a friendly email drafting assistant for new users. First, explain capabilities clearly when asked what you can do. Then gather missing scenario details and draft concise professional emails. If context is incomplete, ask one specific follow-up question.',
    },
  ]
}

function renderAssistantReply(result) {
  const terminalOutput = result.terminal_output
  if (terminalOutput === undefined || terminalOutput === null) return ''
  if (typeof terminalOutput === 'string') return terminalOutput
  return JSON.stringify(terminalOutput, null, 2)
}

function printStepJsonSummary(result) {
  if (!Array.isArray(result.trace) || result.outputs === null || typeof result.outputs !== 'object') {
    return
  }

  for (const node of result.trace) {
    if (typeof node !== 'string') continue
    const nodeValue = result.outputs[node]
    if (nodeValue === null || typeof nodeValue !== 'object') continue
    if (!Object.hasOwn(nodeValue, 'output')) continue
    console.log(`\nStep: ${node}`)
    console.log('JSON')
    console.log(JSON.stringify(nodeValue.output, null, 2))
  }

  if (typeof result.terminal_node === 'string' && result.terminal_output !== undefined && result.terminal_output !== null) {
    console.log(`\nTerminal Step: ${result.terminal_node}`)
    console.log('JSON')
    console.log(JSON.stringify(result.terminal_output, null, 2))
  }
}

function printStreamEvent(event, showThinking, streamState) {
  if (!event || typeof event !== 'object') return
  const eventType = event.event_type
  const isDisplayedStreamEvent = showThinking
    ? eventType === 'node_stream_thinking_delta' || eventType === 'node_stream_output_delta'
    : eventType === 'node_stream_delta'

  if (isDisplayedStreamEvent && typeof event.delta === 'string') {
    const displayNode = typeof event.node_id === 'string' ? event.node_id : typeof event.step_id === 'string' ? event.step_id : 'workflow'
    if (streamState.currentNode !== displayNode) {
      if (streamState.lineOpen) process.stdout.write('\n')
      process.stdout.write(`\nStep: ${displayNode}\n`)
      process.stdout.write('Streaming: ')
      streamState.currentNode = displayNode
      streamState.lineOpen = true
      streamState.lastTokenLabel = null
    }

    if (showThinking) {
      const tokenLabelParts = []
      if (typeof event.token_kind === 'string' && event.token_kind.trim() !== '') {
        tokenLabelParts.push(event.token_kind.trim())
      }
      if (event.is_terminal_node_token === true) tokenLabelParts.push('terminal')
      const tokenLabel = tokenLabelParts.length > 0 ? `[${tokenLabelParts.join(' ')}] ` : ''
      if (tokenLabel !== '' && tokenLabel !== streamState.lastTokenLabel) {
        if (streamState.lineOpen) process.stdout.write('\n')
        process.stdout.write(`${tokenLabel}${displayNode}: `)
        streamState.lastTokenLabel = tokenLabel
        streamState.lineOpen = true
      }
      process.stdout.write(event.delta)
    } else {
      process.stdout.write(event.delta)
    }
  }
}

function parseEventJson(eventJson) {
  if (typeof eventJson !== 'string') return null
  try {
    return JSON.parse(eventJson)
  } catch {
    return null
  }
}

function extractEventJson(firstArg, secondArg) {
  if (typeof secondArg === 'string') return secondArg
  if (typeof firstArg === 'string') return firstArg
  return null
}

function parseResultJson(resultJson) {
  if (typeof resultJson !== 'string') return resultJson
  try {
    return JSON.parse(resultJson)
  } catch {
    return {}
  }
}

function timestampSession() {
  return new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  if (args.showThinking) {
    process.env.SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW = '1'
  } else {
    delete process.env.SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW
  }
  const { provider, apiBase, apiKey } = loadConfig()
  mapProviderEnv(provider, apiKey, apiBase)

  const { Client } = require('../../../crates/simple-agents-napi')
  const client = new Client(provider)

  const workflowPath = resolveWorkflowPath(args.workflow)
  const messages = initialMessages()
  const traceDir = path.resolve(process.cwd(), args.traceDir)
  fs.mkdirSync(traceDir, { recursive: true })
  const traceFile = path.join(traceDir, `chat-session-${timestampSession()}.jsonl`)

  console.log('Chat Email Assistant')
  console.log("Type your request. Type 'exit' to quit.\n")
  console.log(`Trace log: ${traceFile}\n`)

  const rl = readline.createInterface({ input: stdin, output: stdout })
  let interviewClosed = false

  try {
    for (let turn = 1; turn <= args.maxTurns; turn += 1) {
      const rawInput = await rl.question('You: ')
      const userInput = rawInput.trim()
      if (userInput === '') continue

      const lowered = userInput.toLowerCase()
      if (lowered === 'exit' || lowered === 'quit') {
        console.log('Bye!')
        return
      }

      if (interviewClosed) {
        console.log('\nAssistant: This interview session is already closed after termination. Please start a new session with a new run.\n')
        continue
      }

      messages.push({ role: 'user', content: userInput })

      const workflowInput = {
        email_text: userInput,
        messages,
      }

      const streamedEvents = []
      const streamState = { currentNode: null, lineOpen: false, lastTokenLabel: null }
      const result = args.stream
        ? parseResultJson(
            await client.runWorkflowYamlStream(
            workflowPath,
            workflowInput,
            (errOrEventJson, maybeEventJson) => {
              const eventJson = extractEventJson(errOrEventJson, maybeEventJson)
              if (eventJson === null) return
              const event = parseEventJson(eventJson)
              if (event === null) return
              streamedEvents.push(event)
              printStreamEvent(event, args.showThinking, streamState)
            },
          ))
        : args.includeEvents
          ? client.runWorkflowYamlWithEvents(workflowPath, workflowInput)
          : client.runWorkflowYaml(workflowPath, workflowInput)

      if (args.stream) {
        if (streamState.lineOpen) {
          process.stdout.write('\n')
        }
        const expectedEventTypes = args.showThinking
          ? new Set(['node_stream_thinking_delta', 'node_stream_output_delta'])
          : new Set(['node_stream_delta'])
        const hasVisibleStreamEvents = streamedEvents.some(
          (event) => event && expectedEventTypes.has(event.event_type),
        )
        if (!hasVisibleStreamEvents) {
          console.log(`[stream] No ${Array.from(expectedEventTypes).join(', ')} events observed. Ensure llm_call nodes are configured with stream=true.`)
        }
      }

      if (args.showStepJson) {
        printStepJsonSummary(result)
      }

      const traceRecord = {
        timestamp: new Date().toISOString(),
        turn,
        workflow_path: workflowPath,
        workflow_id: result.workflow_id,
        terminal_node: result.terminal_node,
        trace: result.trace || [],
        step_timings: result.step_timings || [],
        total_elapsed_ms: result.total_elapsed_ms,
        user_input: userInput,
        assistant_output: result.terminal_output,
        events: args.stream ? streamedEvents : args.includeEvents ? result.events || [] : null,
      }
      fs.appendFileSync(traceFile, `${JSON.stringify(traceRecord)}\n`, 'utf8')

      const reply = renderAssistantReply(result)
      console.log(`\nAssistant: ${reply}\n`)
      messages.push({ role: 'assistant', content: reply })

      const terminalOutput =
        result.terminal_output !== null && typeof result.terminal_output === 'object'
          ? result.terminal_output
          : {}

      if (
        result.terminal_node === 'terminate_candidate' ||
        result.terminal_node === 'already_terminated' ||
        terminalOutput.decision === 'terminated'
      ) {
        interviewClosed = true
        console.log('Interview closed for this session. Start a new run for a new candidate.\n')
      }

      if (result.terminal_node === 'generate_email_draft') {
        console.log("Draft ready. Continue chatting to refine, or type 'exit'.\n")
      }
    }
  } finally {
    rl.close()
  }

  console.log('Reached max turns. Restart to continue.')
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
