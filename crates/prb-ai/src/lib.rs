//! AI-powered packet explanation engine for Probe.
//!
//! This crate provides LLM-powered plain-English explanations of decoded network events,
//! grounded in structured protocol data to minimize hallucination. Supports privacy-first
//! local models via Ollama and optional cloud providers (OpenAI, custom endpoints).
//!
//! # Architecture
//!
//! - `config`: AI provider configuration (Ollama, OpenAI, custom)
//! - `context`: Converts DebugEvents into structured summaries for LLM consumption
//! - `prompt`: Protocol-specific system prompts with RFC grounding
//! - `explain`: Main engine that orchestrates prompt building and LLM calls
//! - `error`: Error types for AI operations
//!
//! # Example
//!
//! ```no_run
//! use prb_ai::{AiConfig, AiProvider, explain_event};
//! use prb_core::DebugEvent;
//!
//! # async fn example(events: Vec<DebugEvent>) -> Result<(), Box<dyn std::error::Error>> {
//! let config = AiConfig::for_provider(AiProvider::Ollama);
//! let explanation = explain_event(&events, 0, &config).await?;
//! println!("{}", explanation);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(missing_docs)] // TODO: Complete documentation in future segment

pub mod config;
pub mod context;
pub mod error;
pub mod explain;
pub mod prompt;

pub use config::{AiConfig, AiProvider};
pub use context::ExplainContext;
pub use error::AiError;
pub use explain::{explain_event, explain_event_stream};
