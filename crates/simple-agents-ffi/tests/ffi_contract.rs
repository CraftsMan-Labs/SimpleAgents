use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use simple_agents_ffi::{
    sa_client_free, sa_client_new, sa_complete, sa_last_error_message, sa_stream, sa_string_free,
};

// ---------------------------------------------------------------------------
// sa_client_new / sa_client_free
// ---------------------------------------------------------------------------

#[test]
fn rejects_null_api_key() {
    let client = unsafe { sa_client_new(std::ptr::null(), std::ptr::null(), std::ptr::null()) };
    assert!(client.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn rejects_empty_api_key() {
    let empty = CString::new("").unwrap();
    let client = unsafe { sa_client_new(empty.as_ptr(), std::ptr::null(), std::ptr::null()) };
    assert!(client.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn allows_freeing_null_client() {
    unsafe { sa_client_free(std::ptr::null_mut()) };
}

// ---------------------------------------------------------------------------
// sa_complete
// ---------------------------------------------------------------------------

#[test]
fn rejects_null_client_for_complete() {
    let request =
        CString::new(r#"{"model":"gpt-4","messages":[{"role":"user","content":"hi"}]}"#).unwrap();
    let response = unsafe { sa_complete(std::ptr::null_mut(), request.as_ptr()) };
    assert!(response.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn rejects_invalid_request_json() {
    let api_key = CString::new("sk-test-12345678901234567890").unwrap();
    let client = unsafe { sa_client_new(api_key.as_ptr(), std::ptr::null(), std::ptr::null()) };
    if client.is_null() {
        // Provider creation failed (expected in CI without a live key) — test the null check only.
        return;
    }

    let bad_json = CString::new("not-valid-json").unwrap();
    let response = unsafe { sa_complete(client, bad_json.as_ptr()) };
    assert!(response.is_null());

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
    unsafe { sa_client_free(client) };
}

// ---------------------------------------------------------------------------
// sa_stream
// ---------------------------------------------------------------------------

#[test]
fn rejects_null_client_for_stream() {
    let request = CString::new(
        r#"{"model":"gpt-4","messages":[{"role":"user","content":"hi"}],"stream":true}"#,
    )
    .unwrap();
    let result = unsafe {
        sa_stream(
            std::ptr::null_mut(),
            request.as_ptr(),
            None,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(result, 0);

    let err = sa_last_error_message();
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

// ---------------------------------------------------------------------------
// header contract fixture
// ---------------------------------------------------------------------------

#[test]
fn ffi_header_follows_shared_contract_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root should resolve");

    let fixture_path = root.join("parity-fixtures").join("binding_contract.json");
    let fixture_raw = fs::read_to_string(&fixture_path).expect("fixture should be readable");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture_raw).expect("fixture should parse");

    let shared = fixture["shared_cases"]
        .as_object()
        .expect("shared_cases should be an object");
    assert!(shared.contains_key("request"));
    assert!(shared.contains_key("response"));
    assert!(shared.contains_key("healing"));
    assert!(shared.contains_key("streaming"));
    assert!(shared.contains_key("tool_call"));

    let required_symbols = fixture["ffi"]["required_c_symbols"]
        .as_array()
        .expect("ffi.required_c_symbols should be an array");

    let header_path = root
        .join("crates")
        .join("simple-agents-ffi")
        .join("include")
        .join("simple_agents.h");
    let header = fs::read_to_string(header_path).expect("header should be readable");

    for symbol_val in required_symbols {
        let symbol = symbol_val.as_str().expect("symbol should be a string");
        assert!(
            header.contains(symbol),
            "simple_agents.h should include symbol: {symbol}"
        );
    }
}
