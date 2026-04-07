'use strict';

/**
 * Parse a workflow stream event JSON line from `Client.executeWorkflowYamlStream`.
 * @param {string} eventJson
 * @returns {import('./workflow_event').WorkflowRunnerEvent}
 */
function parseWorkflowEvent(eventJson) {
  if (typeof eventJson !== 'string') {
    throw new TypeError('parseWorkflowEvent: expected string');
  }
  return JSON.parse(eventJson);
}

/**
 * Ready-made `onEvent` callback for {@link Client.streamWorkflow} that prints
 * streamed tokens to stdout and silences lifecycle events.
 *
 * Usage:
 * ```js
 * const { defaultOnEvent } = require('simple-agents-node/workflow_event');
 * await client.streamWorkflow(path, input, defaultOnEvent);
 * ```
 *
 * @param {unknown} err
 * @param {string} eventJson
 */
function defaultOnEvent(err, eventJson) {
  if (err) return;
  const event = parseWorkflowEvent(eventJson);
  const eventType = event.event_type;
  const delta = event.delta;
  if (
    (eventType === 'node_stream_delta' ||
      eventType === 'node_stream_thinking_delta' ||
      eventType === 'node_stream_output_delta') &&
    typeof delta === 'string'
  ) {
    process.stdout.write(delta);
  }
}

module.exports = { parseWorkflowEvent, defaultOnEvent };
