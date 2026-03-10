//! Parallel pipeline traits and orchestration.
//!
//! This module defines the traits for pipeline stages and provides
//! orchestration for parallel packet processing.

mod detect;
mod normalize;
mod orchestrator;
mod partition;
mod shard;

pub use detect::{detect_protocol, DetectedProtocol};
pub use normalize::{process_fragments, NormalizeBatch};
pub use orchestrator::{ParallelPipeline, PipelineConfig};
pub use partition::FlowPartitioner;
pub use shard::ShardProcessor;

/// Stateless batch processing stage.
///
/// Stages implementing this trait can process batches of items independently
/// and can be safely shared across rayon threads using `&self`.
///
/// Examples: packet normalization, TLS decryption, protocol decoding.
pub trait BatchStage: Send + Sync {
    /// Input item type (must be `Send` for rayon).
    type Input: Send;
    /// Output item type (must be `Send` for rayon).
    type Output: Send;

    /// Processes a batch of inputs and produces outputs.
    ///
    /// The implementation should be stateless or use only shared immutable state.
    /// Multiple threads may call this concurrently.
    fn process_batch(&self, input: Vec<Self::Input>) -> Vec<Self::Output>;
}

/// Stateful stream processing stage.
///
/// Stages implementing this trait process items one at a time and maintain
/// internal state. Each shard gets its own instance.
///
/// Examples: TCP reassembly, IP fragment collection.
pub trait StreamStage: Send {
    /// Input item type (must be `Send` for cross-shard transfer).
    type Input: Send;
    /// Output item type (must be `Send` for cross-shard transfer).
    type Output: Send;

    /// Processes a single input item and produces zero or more outputs.
    ///
    /// The stage may buffer data internally and return an empty vector
    /// if more input is needed.
    fn process_one(&mut self, input: Self::Input) -> Vec<Self::Output>;

    /// Flushes any buffered state and returns remaining outputs.
    ///
    /// Called at the end of processing to emit any pending data.
    fn flush(&mut self) -> Vec<Self::Output>;
}
