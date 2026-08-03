//! Implementation of Spiess, H. and Florian, M. (1989) "Optimal strategies:
//! A new assignment model for transit networks".
//! See the ref. at spiess_floarian.tex LaTeX file.
//!
//! # Two API tiers
//!
//! The same algorithm is offered through two interfaces; they produce
//! identical results, pick by use case.
//!
//! 1. Simple string API - [`compute_sf`], [`find_optimal_strategy`],
//!    [`assign_demand`]. Nodes are `&str` names, the OD is
//!    `HashMap<origin, HashMap<dest, f64>>`, and results come back as
//!    string-keyed maps ([`Strategy::labels`], [`Volumes::links`]). One call,
//!    nothing to set up. This is the reference / debugging path: it is the
//!    easiest to read, and setting [`VERBOSE`] to `true` prints a step-by-step
//!    trace of both phases. Internally it already uses the same integer arena
//!    as the fast path, so a single solve is fast. Use it for one-off or
//!    single-destination solves, small networks, debugging.
//!
//! 2. Arena API - [`Graph`], [`Workspace`], [`Workspace::assign`],
//!    [`Workspace::solve_each`]. [`Graph::new`] interns the network into an
//!    immutable integer arena once; a [`Workspace`] holds reusable buffers so
//!    each destination is assigned with no further allocation, and results are
//!    returned in integer (arena) indexing. Use it for assigning many
//!    destinations, large networks, and multi-threaded services. On a full
//!    assignment (every stop a destination) it is roughly an order of magnitude
//!    faster than calling [`compute_sf`] per destination, and allocation-free
//!    once the workspace is warm.
//!
//! # Concurrency
//!
//! A [`Graph`] is immutable and `Sync`, so it can be shared across threads by
//! shared reference; a [`Workspace`] is mutated through `&mut self`, so the
//! borrow checker guarantees each thread uses its own. Build the graph once and
//! give each thread its own workspace (see the example on [`Graph`]). The
//! [`DestResult`] returned by `assign` / `solve_each` borrows the workspace and
//! is reused on the next call, which the borrow checker also enforces.

mod demand;
mod hyperpath;
mod hyperpath_queue;
mod solver;
mod spiess_floarian;
mod transit_network;

#[cfg(test)]
mod golden_test;

/// Synthetic-network helpers shared by the unit tests and the `bench` example.
/// Not part of the shipped API: compiled only under `cfg(test)` or the
/// `testutil` feature.
#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

pub use demand::{Volumes, assign_demand};
pub use hyperpath::{Strategy, VERBOSE, find_optimal_strategy};
pub use solver::{DestResult, Graph, Workspace};
pub use spiess_floarian::{SFResult, compute_sf};
pub use transit_network::Link;
