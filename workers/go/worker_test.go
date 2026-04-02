package main

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/dynamicpb"
)

func fieldByName(t *testing.T, md protoreflect.MessageDescriptor, name string) protoreflect.FieldDescriptor {
	t.Helper()
	fd := md.Fields().ByName(protoreflect.Name(name))
	if fd == nil {
		t.Fatalf("missing field %s", name)
	}
	return fd
}

func TestWorkerSmoke(t *testing.T) {
	go func() {
		_ = run("127.0.0.1:50082", "go-smoke")
	}()
	time.Sleep(300 * time.Millisecond)

	fd, err := buildWorkerFileDescriptor()
	if err != nil {
		t.Fatalf("descriptor error: %v", err)
	}
	svc := fd.Services().ByName("WorkerService")

	conn, err := grpc.Dial("127.0.0.1:50082", grpc.WithInsecure())
	if err != nil {
		t.Fatalf("dial error: %v", err)
	}
	defer conn.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	healthReq := dynamicpb.NewMessage(svc.Methods().ByName("Health").Input())
	healthResp := dynamicpb.NewMessage(svc.Methods().ByName("Health").Output())
	if err := conn.Invoke(ctx, "/workflow.worker.v1.WorkerService/Health", healthReq, healthResp); err != nil {
		t.Fatalf("health invoke failed: %v", err)
	}

	executeReq := dynamicpb.NewMessage(svc.Methods().ByName("Execute").Input())
	_ = fieldByName(t, executeReq.Descriptor(), "metadata")
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "request_id"), protoreflect.ValueOfString("smoke-1"))
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "workflow_name"), protoreflect.ValueOfString("wf"))
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "node_id"), protoreflect.ValueOfString("node-1"))
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "operation"), protoreflect.ValueOfString("tool"))
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "target"), protoreflect.ValueOfString("echo"))
	legacyPayload, _ := json.Marshal(map[string]any{"input": map[string]any{"x": 1}})
	executeReq.Set(fieldByName(t, executeReq.Descriptor(), "payload_json"), protoreflect.ValueOfString(string(legacyPayload)))
	toolPayloadField := fieldByName(t, executeReq.Descriptor(), "tool_payload")
	toolPayloadMsg := dynamicpb.NewMessage(toolPayloadField.Message())
	toolInput, _ := json.Marshal(map[string]any{"x": 1})
	toolScopedInput, _ := json.Marshal(map[string]any{"input": map[string]any{"x": 1}})
	toolPayloadMsg.Set(fieldByName(t, toolPayloadMsg.Descriptor(), "input_json"), protoreflect.ValueOfString(string(toolInput)))
	toolPayloadMsg.Set(fieldByName(t, toolPayloadMsg.Descriptor(), "scoped_input_json"), protoreflect.ValueOfString(string(toolScopedInput)))
	executeReq.Set(toolPayloadField, protoreflect.ValueOfMessage(toolPayloadMsg))

	executeResp := dynamicpb.NewMessage(svc.Methods().ByName("Execute").Output())
	if err := conn.Invoke(ctx, "/workflow.worker.v1.WorkerService/Execute", executeReq, executeResp); err != nil {
		t.Fatalf("execute invoke failed: %v", err)
	}
	if !executeResp.Get(fieldByName(t, executeResp.Descriptor(), "ok")).Bool() {
		t.Fatalf("execute response not ok")
	}
}
