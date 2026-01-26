use std::ffi::CString;

use simple_agents_ffi::{
    sa_client_free, sa_client_new_from_env, sa_complete, sa_last_error_message, sa_string_free,
};

#[test]
fn rejects_unknown_provider() {
    let provider = CString::new("unknown").unwrap();
    let client = unsafe { sa_client_new_from_env(provider.as_ptr()) };
    assert!(client.is_null());

    let err = unsafe { sa_last_error_message() };
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn rejects_null_client() {
    let model = CString::new("gpt-4").unwrap();
    let prompt = CString::new("hello").unwrap();

    let response = unsafe { sa_complete(std::ptr::null_mut(), model.as_ptr(), prompt.as_ptr(), 0, -1.0) };
    assert!(response.is_null());

    let err = unsafe { sa_last_error_message() };
    assert!(!err.is_null());
    unsafe { sa_string_free(err) };
}

#[test]
fn allows_freeing_null_client() {
    unsafe { sa_client_free(std::ptr::null_mut()) };
}
