//! Implementation of the Spiess-Florian algorithm for transit assignment.
//! See the ref. at spiess_floarian.tex LaTeX file.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::hyperpath_queue::PriorityQueue;
use crate::transit_network::Link;

/// Strategy is the optimal strategy as defined in the Spiess-Florian algorithm.
pub struct Strategy<'a> {
    /// u_{i} - expected travel time from node i to destination
    pub labels: HashMap<String, f64>,
    /// f_{i} - combined frequency of attractive links at node i
    pub freqs: HashMap<String, f64>,
    /// \overline{A} - attractive links forming the hyperpath
    pub a_set: Vec<&'a Link>,
}

/// When the first attractive link arrives at a node, f_i = 0 and u_i = +Inf,
/// so f_i * u_i = 0 * Inf = NaN in IEEE 754. The correct mathematical value
/// is 1: the Spiess-Florian expected travel time is
///   u_i = (1 + sum(f_a * (c_a + u_j))) / f_i
/// so the product f_i * u_i = 1 + sum(...). At initialization the sum is
/// empty, leaving f_i * u_i = 1. This constant replaces the NaN.
pub(crate) const ALPHA: f64 = 1.0;

/// Frequency used for on-board (riding) links where headway = 0.
/// Must be finite to avoid Inf * 0 = NaN in the update formula.
/// 1e15 gives an effective wait of 1e-15 time units - negligible.
pub(crate) const INFINITE_FREQUENCY: f64 = 1e15;

pub static VERBOSE: AtomicBool = AtomicBool::new(false);

pub(crate) fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn find_optimal_strategy<'a>(
    all_links: &'a [Link],
    all_stops: &HashSet<String>,
    destination: &str,
) -> Strategy<'a> {
    /* 1.1 Initialization */
    if verbose() {
        println!("1.1 Initialization \\\\");
    }
    let mut u: HashMap<String, f64> = HashMap::with_capacity(all_stops.len());
    let mut f: HashMap<String, f64> = HashMap::with_capacity(all_stops.len());
    for stop in all_stops {
        if verbose() {
            println!("$f_{{{}}} = 0$ \\\\ ", stop);
        }
        f.insert(stop.clone(), 0.0);
        if stop == destination {
            if verbose() {
                println!("$u_{{{}}} = 0$ \\\\ ", destination);
            }
            u.insert(stop.clone(), 0.0);
            continue;
        }
        if verbose() {
            println!("$u_{{{}}} = Infinity$ \\\\ ", stop);
        }
        u.insert(stop.clone(), f64::INFINITY);
    }

    let mut overline_a: Vec<&'a Link> = Vec::with_capacity(all_links.len() / 2);

    let mut links_by_to_node: HashMap<&'a str, Vec<&'a Link>> = HashMap::new();
    for link in all_links {
        links_by_to_node
            .entry(link.to_node.as_str())
            .or_default()
            .push(link);
    }

    let mut entries: HashMap<&'a str, Vec<usize>> = HashMap::with_capacity(all_links.len());
    let mut pq = PriorityQueue::with_capacity(all_links.len());
    for link in all_links {
        let priority = u.get(&link.to_node).copied().unwrap_or(0.0) + link.travel_cost;
        let id = pq.push(link, priority);
        entries
            .entry(link.from_node.as_str())
            .or_default()
            .push(id);
    }
    pq.init();
    if verbose() {
        pq.print();
    }
    while pq.len() > 0 {
        /* 1.2 Get next link */
        if verbose() {
            pq.print();
        }
        let entry_id = match pq.pop() {
            Some(id) => id,
            None => break,
        };
        let priority = pq.priority(entry_id);
        if priority.is_infinite() && priority > 0.0 {
            break;
        }
        let a = pq.link(entry_id);
        let i = a.from_node.as_str();
        let j = a.to_node.as_str();
        let sum_uc = u.get(j).copied().unwrap_or(0.0) + a.travel_cost;

        /* 1.3 Update node label */
        if verbose() {
            println!("Process: $a = (i, j) = ({}, {})$, \\\\ ", i, j);
        }
        let u_i = u.get(i).copied().unwrap_or(0.0);
        if u_i < sum_uc {
            continue;
        }
        let u_j = u.get(j).copied().unwrap_or(0.0);
        if verbose() {
            println!(
                "\\quad $u_i < u_j + c_a : {} < {} + {}$ - FALSE \\\\ ",
                u_i, u_j, a.travel_cost
            );
        }
        let mut freq = INFINITE_FREQUENCY;
        if a.headway > 0.0 {
            freq = 1.0 / a.headway;
        }
        let f_i = f.get(i).copied().unwrap_or(0.0);
        if verbose() {
            println!("\\quad $f_a = {}$ \\\\ ", freq);
            println!("\\quad $u_j + c_a = {}$ \\\\ ", u_j + a.travel_cost);
            println!("\\quad $u_i = {}$ \\\\ ", u_i);
            println!(
                "\\quad$u_i = \\frac{{f_i * u_i + f_a * (u_j + c_a)}}{{f_i + f_a}} = \\frac{{({}) * ({}) + ({}) * (({}) + ({}))}}{{({}) + ({})}} = $ \\\\ ",
                f_i, u_i, freq, u_j, a.travel_cost, f_i, freq
            );
        }
        let mut numerator_part = f_i * u_i;
        if numerator_part.is_nan() {
            numerator_part = ALPHA;
        }
        let mut numerator_part2 = freq * (u_j + a.travel_cost);
        if numerator_part2.is_nan() {
            numerator_part2 = ALPHA;
        }
        let numerator = numerator_part + numerator_part2;
        let denominator = f_i + freq;
        u.insert(i.to_string(), numerator / denominator);
        if verbose() {
            println!(
                "\\quad \\quad $\\frac{{({}) + ({})}}{{({}) + ({})}} = \\frac{{{}}}{{{}}} = {}$ \\\\ ",
                numerator_part,
                numerator_part2,
                f_i,
                freq,
                numerator,
                denominator,
                u.get(i).copied().unwrap_or(0.0)
            );
            println!(
                "\\quad $f_i = f_{{i}} + f_a = ({}) + ({}) = {}$ \\\\ ",
                f_i, freq, denominator
            );
            println!(
                "\\quad $\\overline{{A}} = \\overline{{A}} \\cup {{a}} = \\overline{{A}} \\cup {{({}, {})}}$ \\\\ ",
                i, j
            );
        }
        f.insert(i.to_string(), denominator);

        overline_a.push(a);

        if let Some(links_to_update) = links_by_to_node.get(i) {
            for link in links_to_update {
                if let Some(i_entries) = entries.get(link.from_node.as_str()) {
                    for &eid in i_entries {
                        let entry_link = pq.link(eid);
                        if entry_link.to_node == i && entry_link.from_node == link.from_node {
                            let new_priority =
                                u.get(i).copied().unwrap_or(0.0) + link.travel_cost;
                            pq.update(eid, new_priority);
                            break;
                        }
                    }
                }
            }
        }
        if verbose() {
            println!("Node labels: \\\\");
            for s in all_stops {
                println!(
                    "${} -> (u_i, f_i) = ({}, {})$ \\\\ ",
                    s,
                    u.get(s).copied().unwrap_or(0.0),
                    f.get(s).copied().unwrap_or(0.0)
                );
            }
        }
    }
    Strategy {
        labels: u,
        freqs: f,
        a_set: overline_a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyper_paths() {
        VERBOSE.store(true, Ordering::Relaxed);
        let all_nodes: HashSet<String> = ["A", "X", "X2", "Y", "Y3", "B"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all_links = vec![
            Link::new("A", "B", "Line 1", 25.0, 6.0),
            Link::new("A", "X2", "Line 2", 7.0, 6.0),
            Link::new("X2", "X", "Line 2", 0.0, 0.0),
            Link::new("X", "X2", "Line 2", 0.0, 6.0),
            Link::new("X2", "Y", "Line 2", 6.0, 0.0),
            Link::new("Y3", "Y", "Line 3", 0.0, 15.0),
            Link::new("Y", "B", "Line 4", 10.0, 3.0),
            Link::new("X", "Y3", "Line 3", 4.0, 15.0),
            Link::new("Y", "Y3", "Line 3", 0.0, 15.0),
            Link::new("Y3", "B", "Line 3", 4.0, 0.0),
        ];
        let destination_node = "B";
        let ops = find_optimal_strategy(&all_links, &all_nodes, destination_node);

        const EPS: f64 = 1e-9;

        let expected_labels: HashMap<&str, f64> = HashMap::from([
            ("A", 27.75),
            ("X", 19.071428571428573),
            ("X2", 17.5),
            ("Y", 11.5),
            ("Y3", 4.0),
            ("B", 0.0),
        ]);
        let expected_freqs: HashMap<&str, f64> = HashMap::from([
            ("A", 1.0 / 3.0),
            ("X", 7.0 / 30.0),
            ("X2", INFINITE_FREQUENCY),
            ("Y", 0.4),
            ("Y3", INFINITE_FREQUENCY),
            ("B", 0.0),
        ]);
        // Matches the paper order (Spiess & Florian 1989, p. 93-94)
        let expected_a_set: Vec<&Link> = vec![
            // Y3->B
            &all_links[9],
            // Y->Y3
            &all_links[8],
            // X->Y3
            &all_links[7],
            // Y->B
            &all_links[6],
            // X2->Y
            &all_links[4],
            // X->X2
            &all_links[3],
            // A->X2
            &all_links[1],
            // A->B
            &all_links[0],
        ];

        assert_eq!(
            ops.labels.len(),
            expected_labels.len(),
            "Incorrect number of labels"
        );
        assert_eq!(
            ops.freqs.len(),
            expected_freqs.len(),
            "Incorrect number of frequencies"
        );
        assert_eq!(
            ops.a_set.len(),
            expected_a_set.len(),
            "Incorrect number of links in attractive set"
        );

        for (k, v) in &ops.labels {
            assert!(
                expected_labels.contains_key(k.as_str()),
                "Incorrect label key {} has met",
                k
            );
            let want = expected_labels[k.as_str()];
            assert!(
                (v - want).abs() <= EPS,
                "Incorrect label value for node {}: got {}, want {}",
                k,
                v,
                want
            );
        }
        for (k, v) in &ops.freqs {
            assert!(
                expected_freqs.contains_key(k.as_str()),
                "Incorrect frequency key {} has met",
                k
            );
            let want = expected_freqs[k.as_str()];
            assert!(
                (v - want).abs() <= EPS,
                "Incorrect frequency value for node {}: got {}, want {}",
                k,
                v,
                want
            );
        }
        for (i, v) in ops.a_set.iter().enumerate() {
            println!("{:?} {:?}", v, expected_a_set[i]);
            assert!(
                std::ptr::eq(*v, expected_a_set[i]),
                "Incorrect link in attractive set at index {}",
                i
            );
        }
    }
}
