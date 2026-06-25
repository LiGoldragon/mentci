//! Mentci daemon runtime.
//!
//! `signal-mentci` owns the programmable UI contract. This crate owns the
//! daemon process: startup configuration, canonical in-memory state for this
//! first slice, and a Unix-socket server over length-prefixed rkyv frames.

pub mod client;
pub mod command;
pub mod configuration;
pub mod criome_bridge;
pub mod daemon;
pub mod error;
pub mod frame_codec;
pub mod harness_liveness;
pub mod harness_sessions;
pub mod introspection_bridge;
pub mod preflight;
pub mod state;

pub use error::{Error, Result};
