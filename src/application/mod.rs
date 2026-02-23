//! # Application Layer
//!
//! Use cases and orchestration logic. This layer coordinates domain
//! ports to implement the full request lifecycle:
//!
//! `UserRequest` → Planner → Router → Queue → Agents → Aggregator → `AggregateResponse`

pub mod aggregator;
pub mod circuit_breaker;
pub mod context_service;
pub mod orchestrator_service;
pub mod planner;
pub mod router;
