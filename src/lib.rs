//! Implementation of Spiess, H. and Florian, M. (1989) "Optimal strategies:
//! A new assignment model for transit networks".
//! See the ref. at spiess_floarian.tex LaTeX file.

mod demand;
mod hyperpath;
mod hyperpath_queue;
mod spiess_floarian;
mod transit_network;

pub use demand::{Volumes, assign_demand};
pub use hyperpath::{Strategy, VERBOSE, find_optimal_strategy};
pub use spiess_floarian::{SFResult, compute_sf};
pub use transit_network::Link;
