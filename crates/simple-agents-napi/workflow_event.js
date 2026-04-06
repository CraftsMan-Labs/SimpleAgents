'use strict';

/**
 * Parse a workflow stream event JSON line from `Client.executeWorkflowYamlStream`.
 * @param {string} eventJson
 * @returns {Record<string, unknown>}
 */
function parseWorkflowEvent(eventJson) {
  if (typeof eventJson !== 'string') {
    throw new TypeError('parseWorkflowEvent: expected string');
  }
  return JSON.parse(eventJson);
}

module.exports = { parseWorkflowEvent };
