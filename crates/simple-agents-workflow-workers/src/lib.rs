//! gRPC worker contracts and pool integration.

pub mod client;
pub mod pool;

pub mod proto {
    tonic::include_proto!("workflow.worker.v1");
}

pub use client::GrpcWorkerClient;
pub use pool::{GrpcWorkerPool, GrpcWorkerPoolOptions};
