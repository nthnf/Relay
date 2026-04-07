//! Shared protobuf and gRPC contract crate.

pub mod bootstrap {
    tonic::include_proto!("relay.bootstrap");
}

pub mod chat {
    tonic::include_proto!("relay.chat");
}

pub mod friendship {
    tonic::include_proto!("relay.friendship");
}

pub mod identity {
    tonic::include_proto!("relay.identity");
}

pub mod realtime {
    tonic::include_proto!("relay.realtime");
}

pub mod workspace {
    tonic::include_proto!("relay.workspace");
}
