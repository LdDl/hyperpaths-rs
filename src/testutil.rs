//! Synthetic-network helpers shared by the unit tests and the `bench` example.
//! Compiled only under `cfg(test)` or the `testutil` feature; not shipped API.

use std::collections::{HashMap, HashSet};

use crate::transit_network::Link;

/// An origin -> destination -> demand matrix.
pub type Od = HashMap<String, HashMap<String, f64>>;

/// A synthetic grid network: links, node set, destination and OD.
pub type GridNetwork = (Vec<Link>, HashSet<String>, String, Od);

/// Builds a synthetic expanded transit route graph on a rows x cols grid of
/// stops: an eastbound line per row and a southbound line per column, each
/// expanded into boarding / riding / alighting links. The bottom-right stop is
/// the destination and reachable from every stop, and shared grid stops create
/// genuine common-line choices, so the hyperpath is non-trivial.
///
/// Returns the links, node set, destination and an OD loading one trip from
/// every other stop to the destination.
pub fn gen_grid_network(
    rows: usize,
    cols: usize,
    headway: f64,
    seg_time: f64,
) -> GridNetwork {
    let stop = |r: usize, c: usize| format!("s_{}_{}", r, c);
    let mut nodes: HashSet<String> = HashSet::new();
    let mut links: Vec<Link> = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            nodes.insert(stop(r, c));
        }
    }

    let add_line = |id: &str, seq: &[String], links: &mut Vec<Link>, nodes: &mut HashSet<String>| {
        let platform = |i: usize| format!("{}#{}", id, i);
        for i in 0..seq.len() {
            nodes.insert(platform(i));
            if i < seq.len() - 1 {
                links.push(Link::new(&seq[i], &platform(i), id, 0.0, headway));
                links.push(Link::new(&platform(i), &platform(i + 1), id, seg_time, 0.0));
            }
            if i > 0 {
                links.push(Link::new(&platform(i), &seq[i], id, 0.0, 0.0));
            }
        }
    };

    for r in 0..rows {
        let seq: Vec<String> = (0..cols).map(|c| stop(r, c)).collect();
        add_line(&format!("R{}", r), &seq, &mut links, &mut nodes);
    }
    for c in 0..cols {
        let seq: Vec<String> = (0..rows).map(|r| stop(r, c)).collect();
        add_line(&format!("C{}", c), &seq, &mut links, &mut nodes);
    }

    let dest = stop(rows - 1, cols - 1);
    let mut od: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for r in 0..rows {
        for c in 0..cols {
            let s = stop(r, c);
            if s == dest {
                continue;
            }
            od.insert(s, HashMap::from([(dest.clone(), 1.0)]));
        }
    }
    (links, nodes, dest, od)
}

/// The stop node names of a rows x cols grid, in row-major order.
pub fn grid_stops(rows: usize, cols: usize) -> Vec<String> {
    let mut stops = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            stops.push(format!("s_{}_{}", r, c));
        }
    }
    stops
}

/// A full OD: one trip from every stop to every other stop.
pub fn grid_full_od(stops: &[String]) -> Od {
    let mut od = HashMap::with_capacity(stops.len());
    for o in stops {
        let mut row = HashMap::with_capacity(stops.len().saturating_sub(1));
        for d in stops {
            if d != o {
                row.insert(d.clone(), 1.0);
            }
        }
        od.insert(o.clone(), row);
    }
    od
}
