package simpleagents

/*
#cgo CFLAGS: -I${SRCDIR}/../../crates/simple-agents-ffi/include
#cgo LDFLAGS: -lsimple_agents_ffi

#include <stdlib.h>
#include "simple_agents.h"
*/
import "C"

import (
	"errors"
	"unsafe"
)

type Client struct {
	ptr *C.SAClient
}

func NewClientFromEnv(provider string) (*Client, error) {
	cProvider := C.CString(provider)
	defer C.free(unsafe.Pointer(cProvider))

	ptr := C.sa_client_new_from_env(cProvider)
	if ptr == nil {
		return nil, lastError()
	}

	return &Client{ptr: ptr}, nil
}

func (c *Client) Close() {
	if c == nil || c.ptr == nil {
		return
	}
	C.sa_client_free(c.ptr)
	c.ptr = nil
}

func (c *Client) Complete(model, prompt string, maxTokens int32, temperature float32) (string, error) {
	if c == nil || c.ptr == nil {
		return "", errors.New("client is not initialized")
	}

	cModel := C.CString(model)
	defer C.free(unsafe.Pointer(cModel))
	cPrompt := C.CString(prompt)
	defer C.free(unsafe.Pointer(cPrompt))

	response := C.sa_complete(c.ptr, cModel, cPrompt, C.int32_t(maxTokens), C.float(temperature))
	if response == nil {
		return "", lastError()
	}
	defer C.sa_string_free(response)

	return C.GoString(response), nil
}

func lastError() error {
	msg := C.sa_last_error_message()
	if msg == nil {
		return errors.New("unknown error")
	}
	defer C.sa_string_free(msg)
	return errors.New(C.GoString(msg))
}
