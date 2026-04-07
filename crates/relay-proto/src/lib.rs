//! Shared protobuf and gRPC contract crate.

pub mod identity {
    tonic::include_proto!("relay.identity");
}
