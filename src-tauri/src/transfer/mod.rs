pub mod manager;

pub use manager::{
    generate_renamed_path, ChannelTransferEventSink, NoopTransferEventSink, TauriTransferEventSink,
    TransferEventSink, TransferManager,
};
