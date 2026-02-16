package main

import "fmt"

func getRagData(topic string, emailText string, contextNodes int) map[string]any {
	data := map[string][2]string{
		"probation": {
			"hr_policy/probation.md",
			"Collect manager review, performance evidence, and probation timeline.",
		},
		"leave_request": {
			"hr_policy/leave.md",
			"Validate leave balance, manager approval, and blackout dates.",
		},
		"supply_chain_order_assessment": {
			"supply_chain/order_assessment.md",
			"Review order specs, inventory risk, and vendor lead-time guidance.",
		},
		"supply_chain_order_replacement": {
			"supply_chain/order_replacement.md",
			"Collect order id, damage proof, and replacement SLA policy.",
		},
		"termination_first_time_offense": {
			"hr_policy/termination_first_offense.md",
			"Validate first-incident criteria and route to HRBP review.",
		},
		"termination_repeated_offense": {
			"hr_policy/termination_repeated_offense.md",
			"Collect prior warnings and escalation approvals before final action.",
		},
		"clarification": {
			"shared/request_clarification.md",
			"Request clarifying details before routing.",
		},
	}

	value, ok := data[topic]
	if !ok {
		value = data["clarification"]
	}

	preview := emailText
	if len(preview) > 120 {
		preview = preview[:120]
	}

	return map[string]any{
		"kb_source":     value[0],
		"playbook":      value[1],
		"handler":       "GetRagData",
		"topic":         topic,
		"email_preview": preview,
		"context_nodes": fmt.Sprintf("%d", contextNodes),
	}
}
