package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net"
	"path/filepath"
	"runtime"

	"github.com/jhump/protoreflect/desc/protoparse"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/reflect/protodesc"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/dynamicpb"
)

type workerServer struct {
	workerID        string
	executeReqDesc  protoreflect.MessageDescriptor
	executeRespDesc protoreflect.MessageDescriptor
	workerErrDesc   protoreflect.MessageDescriptor
	healthRespDesc  protoreflect.MessageDescriptor
	healthEnumDesc  protoreflect.EnumDescriptor
}

func newWorkerServer(workerID string, fd protoreflect.FileDescriptor) *workerServer {
	msgs := fd.Messages()
	return &workerServer{
		workerID:        workerID,
		executeReqDesc:  msgs.ByName("ExecuteRequest"),
		executeRespDesc: msgs.ByName("ExecuteResponse"),
		workerErrDesc:   msgs.ByName("WorkerError"),
		healthRespDesc:  msgs.ByName("HealthResponse"),
		healthEnumDesc:  fd.Enums().ByName("HealthStatus"),
	}
}

func (s *workerServer) execute(_ context.Context, req *dynamicpb.Message) (*dynamicpb.Message, error) {
	requestID := req.Get(s.executeReqDesc.Fields().ByName("request_id")).String()
	operation := req.Get(s.executeReqDesc.Fields().ByName("operation")).String()
	target := req.Get(s.executeReqDesc.Fields().ByName("target")).String()
	payloadJSON := req.Get(s.executeReqDesc.Fields().ByName("payload_json")).String()

	resp := dynamicpb.NewMessage(s.executeRespDesc)
	resp.Set(s.executeRespDesc.Fields().ByName("request_id"), protoreflect.ValueOfString(requestID))
	resp.Set(s.executeRespDesc.Fields().ByName("worker_id"), protoreflect.ValueOfString(s.workerID))
	resp.Set(s.executeRespDesc.Fields().ByName("elapsed_ms"), protoreflect.ValueOfUint64(1))

	if target == "fail" {
		errMsg := dynamicpb.NewMessage(s.workerErrDesc)
		errMsg.Set(s.workerErrDesc.Fields().ByName("code"), protoreflect.ValueOfString("execution_failed"))
		errMsg.Set(s.workerErrDesc.Fields().ByName("message"), protoreflect.ValueOfString("forced failure"))
		errMsg.Set(s.workerErrDesc.Fields().ByName("retryable"), protoreflect.ValueOfBool(false))

		resp.Set(s.executeRespDesc.Fields().ByName("ok"), protoreflect.ValueOfBool(false))
		resp.Set(s.executeRespDesc.Fields().ByName("error"), protoreflect.ValueOfMessage(errMsg))
		return resp, nil
	}

	var payload any
	if payloadJSON != "" {
		if err := json.Unmarshal([]byte(payloadJSON), &payload); err != nil {
			payload = map[string]any{"raw": payloadJSON}
		}
	}

	output := map[string]any{
		"language":  "go",
		"worker_id": s.workerID,
		"operation": operation,
		"target":    target,
		"payload":   payload,
	}
	body, _ := json.Marshal(output)

	resp.Set(s.executeRespDesc.Fields().ByName("ok"), protoreflect.ValueOfBool(true))
	resp.Set(s.executeRespDesc.Fields().ByName("output_json"), protoreflect.ValueOfString(string(body)))
	return resp, nil
}

func (s *workerServer) health() *dynamicpb.Message {
	resp := dynamicpb.NewMessage(s.healthRespDesc)
	resp.Set(s.healthRespDesc.Fields().ByName("worker_id"), protoreflect.ValueOfString(s.workerID))
	resp.Set(s.healthRespDesc.Fields().ByName("status"), protoreflect.ValueOfEnum(s.healthEnumDesc.Values().ByName("HEALTH_STATUS_SERVING").Number()))
	resp.Set(s.healthRespDesc.Fields().ByName("consecutive_failures"), protoreflect.ValueOfUint32(0))
	return resp
}

func registerWorkerService(server *grpc.Server, fd protoreflect.FileDescriptor, impl *workerServer) {
	svc := fd.Services().ByName("WorkerService")
	executeMethod := svc.Methods().ByName("Execute")
	healthMethod := svc.Methods().ByName("Health")

	serviceDesc := grpc.ServiceDesc{
		ServiceName: string(svc.FullName()),
		HandlerType: (*interface{})(nil),
		Methods: []grpc.MethodDesc{
			{
				MethodName: string(executeMethod.Name()),
				Handler: func(_ interface{}, ctx context.Context, dec func(any) error, _ grpc.UnaryServerInterceptor) (any, error) {
					req := dynamicpb.NewMessage(executeMethod.Input())
					if err := dec(req); err != nil {
						return nil, err
					}
					return impl.execute(ctx, req)
				},
			},
			{
				MethodName: string(healthMethod.Name()),
				Handler: func(_ interface{}, _ context.Context, dec func(any) error, _ grpc.UnaryServerInterceptor) (any, error) {
					req := dynamicpb.NewMessage(healthMethod.Input())
					if err := dec(req); err != nil {
						return nil, err
					}
					return impl.health(), nil
				},
			},
		},
	}

	server.RegisterService(&serviceDesc, impl)
}

func buildWorkerFileDescriptor() (protoreflect.FileDescriptor, error) {
	_, thisFile, _, ok := runtime.Caller(0)
	if !ok {
		return nil, fmt.Errorf("resolve source path")
	}

	protoPath := filepath.Clean(filepath.Join(
		filepath.Dir(thisFile),
		"../../crates/simple-agents-workflow-workers/proto/worker.proto",
	))
	parser := protoparse.Parser{
		ImportPaths: []string{filepath.Dir(protoPath)},
	}
	files, err := parser.ParseFiles(filepath.Base(protoPath))
	if err != nil {
		return nil, fmt.Errorf("parse worker proto: %w", err)
	}
	if len(files) == 0 {
		return nil, fmt.Errorf("worker proto parse produced no descriptors")
	}

	return protodesc.NewFile(files[0].AsFileDescriptorProto(), nil)
}

func run(listenAddr string, workerID string) error {
	fd, err := buildWorkerFileDescriptor()
	if err != nil {
		return fmt.Errorf("build descriptor: %w", err)
	}

	listener, err := net.Listen("tcp", listenAddr)
	if err != nil {
		return fmt.Errorf("listen: %w", err)
	}

	server := grpc.NewServer()
	registerWorkerService(server, fd, newWorkerServer(workerID, fd))
	log.Printf("go worker listening on %s", listenAddr)
	return server.Serve(listener)
}

func main() {
	listenAddr := flag.String("listen", "127.0.0.1:50062", "listen address")
	workerID := flag.String("worker-id", "go-0", "worker identifier")
	flag.Parse()

	if err := run(*listenAddr, *workerID); err != nil {
		log.Fatal(err)
	}
}
