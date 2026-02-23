//! # OmniShell Orchestrator
//!
//! A production-grade multi-agent AI orchestrator written in Rust.
//!
//! ## Architecture
//!
//! The crate follows Clean Architecture (Hexagonal / Ports & Adapters):
//!
//! - **`domain`**: Pure types, traits (ports), and error definitions.
//!   No dependencies on infrastructure or frameworks.
//! - **`application`**: Use cases and orchestration logic.
//!   Coordinates domain ports to implement the request lifecycle.
//! - **`infrastructure`**: Adapters implementing domain ports.
//!   CLI agent adapters, caches, executors, config loaders, etc.
//! - **`interface`**: User-facing CLI interface.
//!
//! ## Request Flow
//!
//! ```text
//! User → CLI → OrchestratorService
//!   → PolicyGuard (validate input)
//!   → Planner (split into subtasks)
//!   → Router (assign agents by capability)
//!   → Dispatch (parallel execution with concurrency limits)
//!       → Cache check (LRU)
//!       → Agent execution (CLI subprocess)
//!       → Circuit breaker tracking
//!   → Aggregator (combine results)
//!   → ContextManager (update history + trim)
//! → AggregateResponse → User
//! ```
//!
//! ## Security
//!
//! - Prompt injection mitigation via `PolicyGuard`.
//! - Tool execution sandboxing via allowlist/denylist.
//! - Secrets never hardcoded; loaded from environment.
//! - No shell concatenation; all args passed as vectors.
//! - Structured logging with optional redaction.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
