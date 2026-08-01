use std::collections::{HashMap, HashSet};

use crate::demand::{Volumes, assign_demand};
use crate::hyperpath::{Strategy, find_optimal_strategy};
use crate::transit_network::Link;

/// SFResult is the result of running through the Spiess-Florian algorithm
pub struct SFResult<'a> {
    /// Optimal strategy
    pub strategy: Strategy<'a>,
    /// Assigned demand
    pub volumes: Volumes,
}

/// compute_sf computes the Spiess-Florian algorithm
pub fn compute_sf<'a>(
    all_links: &'a [Link],
    all_stops: &HashSet<String>,
    destination: &str,
    od_matrix: &HashMap<String, HashMap<String, f64>>,
) -> SFResult<'a> {
    // Part 1: Find optimal strategy
    let ops = find_optimal_strategy(all_links, all_stops, destination);
    // Part 2: Assign demand according to optimal strategy
    let volumes = assign_demand(all_links, all_stops, &ops, od_matrix, destination);
    SFResult {
        strategy: ops,
        volumes,
    }
}
