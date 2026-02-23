//! # Domain Layer
//!
//! Pure domain types, traits (ports), and error definitions.
//! This layer has zero dependencies on infrastructure or frameworks.
//! All external behavior is defined through traits (Ports & Adapters pattern).

pub mod agent;
pub mod cache;
pub mod context;
pub mod error;
pub mod orchestrator;
pub mod policy;
pub mod secrets;
pub mod task;
pub mod token;
pub mod tool;
