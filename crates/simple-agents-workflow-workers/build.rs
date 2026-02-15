fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to fetch vendored protoc");
    std::env::set_var("PROTOC", protoc);

    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/worker.proto"], &["proto"])
        .expect("failed to compile worker proto");

    println!("cargo:rerun-if-changed=proto/worker.proto");
}
