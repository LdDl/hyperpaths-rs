//! Implementation of Spiess, H. and Florian, M. (1989) "Optimal strategies:
//! A new assignment model for transit networks".
//! See the ref. at spiess_floarian.tex LaTeX file.

mod demand;
mod hyperpath;
mod hyperpath_queue;
mod spiess_floarian;
mod transit_network;

pub use demand::{assign_demand, Volumes};
pub use hyperpath::{find_optimal_strategy, Strategy, VERBOSE};
pub use spiess_floarian::{compute_sf, SFResult};
pub use transit_network::Link;
