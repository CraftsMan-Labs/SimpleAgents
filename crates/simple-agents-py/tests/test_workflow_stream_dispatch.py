"""Dispatcher for ``simple_agents_py.workflow_stream.workflow_event_callback``."""

from __future__ import annotations

import unittest.mock
from typing import Any, Mapping

import pytest

from simple_agents_py.workflow_stream import EVENT_TYPE_TO_METHOD, workflow_event_callback

_HOOK_METHOD_NAMES = list(EVENT_TYPE_TO_METHOD.values())


def _evt(event_type: str, **extra: Any) -> dict[str, Any]:
    return {"event_type": event_type, **extra}


@pytest.mark.parametrize("event_type,method_name", list(EVENT_TYPE_TO_METHOD.items()))
def test_dispatch_calls_mapped_method(event_type: str, method_name: str) -> None:
    hooks = unittest.mock.Mock(spec=_HOOK_METHOD_NAMES)
    cb = workflow_event_callback(hooks)
    payload = _evt(event_type, node_id="n1")
    cb(payload)
    m = getattr(hooks, method_name)
    m.assert_called_once_with(payload)


def test_unknown_event_type_no_op_without_on_event() -> None:
    hooks = unittest.mock.Mock(spec=_HOOK_METHOD_NAMES)
    cb = workflow_event_callback(hooks)
    cb(_evt("node_healed"))
    for name in _HOOK_METHOD_NAMES:
        getattr(hooks, name).assert_not_called()


def test_unknown_event_calls_on_event_when_present() -> None:
    hooks = unittest.mock.Mock()
    cb = workflow_event_callback(hooks)
    payload = _evt("node_healed")
    cb(payload)
    hooks.on_event.assert_called_once_with(payload)


def test_specific_hook_runs_before_on_event() -> None:
    order: list[str] = []

    class Hooks:
        def on_node_started(self, event: Mapping[str, Any]) -> None:
            order.append("on_node_started")

        def on_event(self, event: Mapping[str, Any]) -> None:
            order.append("on_event")

    workflow_event_callback(Hooks())(_evt("node_started"))
    assert order == ["on_node_started", "on_event"]


def test_only_on_event_when_specific_hook_missing() -> None:
    order: list[str] = []

    class Hooks:
        def on_event(self, event: Mapping[str, Any]) -> None:
            order.append("on_event")

    workflow_event_callback(Hooks())(_evt("workflow_completed"))
    assert order == ["on_event"]
