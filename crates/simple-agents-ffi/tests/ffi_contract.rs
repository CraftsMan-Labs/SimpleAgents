use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use simple_agents_ffi::{
    sa_client_free, sa_client_new_from_env, sa_complete, sa_complete_messages_json,
    sa_last_error_message, sa_stream_messages, sa_string_free, SAMessage,
};

#[derive(Debug, Deserialize)]
struct FfiContractFixture {
    ffi: FfiSymbols,
    shared_cases: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct FfiSymbols {
    required_c_symbols: Vec<String>,
}

#[test]
fn rejects_unknown_provider() {
    let provider = CString::new("unknown").unwrap();
    let client = unsafe { sa_client_new_from_env(provider.as_ptr()) };
    assert!(client.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn rejects_null_client() {
    let model = CString::new("gpt-4").unwrap();
    let prompt = CString::new("hello").unwrap();

    let response = unsafe {
        sa_complete(
            std::ptr::null_mut(),
            model.as_ptr(),
            prompt.as_ptr(),
            0,
            -1.0,
        )
    };
    assert!(response.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn allows_freeing_null_client() {
    unsafe { sa_client_free(std::ptr::null_mut()) };
}

#[test]
fn rejects_null_client_for_messages_json() {
    let model = CString::new("gpt-4").unwrap();
    let role = CString::new("user").unwrap();
    let content = CString::new("hello").unwrap();
    let messages = [SAMessage {
        role: role.as_ptr(),
        content: content.as_ptr(),
        name: std::ptr::null(),
        tool_call_id: std::ptr::null(),
    }];

    let response = unsafe {
        sa_complete_messages_json(
            std::ptr::null_mut(),
            model.as_ptr(),
            messages.as_ptr(),
            messages.len(),
            0,
            -1.0,
            -1.0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert!(response.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn rejects_empty_messages() {
    let provider_name = if std::env::var("OPENAI_API_KEY").is_ok() {
        Some("openai")
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        Some("anthropic")
    } else {
        None
    };
    let Some(provider_name) = provider_name else {
        return;
    };

    let provider = CString::new(provider_name).unwrap();
    let client = unsafe { sa_client_new_from_env(provider.as_ptr()) };
    assert!(!client.is_null());

    let model = CString::new("gpt-4").unwrap();
    let response = unsafe {
        sa_complete_messages_json(
            client,
            model.as_ptr(),
            std::ptr::null(),
            0,
            0,
            -1.0,
            -1.0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert!(response.is_null());
    unsafe { sa_client_free(client) };
}

#[test]
fn rejects_null_client_for_stream_messages() {
    let response = unsafe {
        sa_stream_messages(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            -1.0,
            -1.0,
            None,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(response, 0);

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn ffi_header_follows_shared_contract_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root should resolve");

    let fixture_path = root.join("parity-fixtures").join("binding_contract.json");
    let fixture_raw = fs::read_to_string(&fixture_path).expect("fixture should be readable");
    let fixture: FfiContractFixture =
        serde_json::from_str(&fixture_raw).expect("fixture should parse");

    let shared = fixture
        .shared_cases
        .as_object()
        .expect("shared_cases should be an object");
    assert!(shared.contains_key("request"));
    assert!(shared.contains_key("response"));
    assert!(shared.contains_key("healing"));
    assert!(shared.contains_key("streaming"));
    assert!(shared.contains_key("tool_call"));

    let header_path = root
        .join("crates")
        .join("simple-agents-ffi")
        .join("include")
        .join("simple_agents.h");
    let header = fs::read_to_string(header_path).expect("header should be readable");
    for symbol in fixture.ffi.required_c_symbols {
        assert!(
            header.contains(&symbol),
            "simple_agents.h should include symbol: {symbol}"
        );
    }
}
