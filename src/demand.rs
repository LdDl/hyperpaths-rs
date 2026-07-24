use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::hyperpath::{verbose, Strategy, INFINITE_FREQUENCY};
use crate::transit_network::Link;

/// Volumes holds the assigned demand according to the optimal strategy.
pub struct Volumes {
    /// Link volumes: links[from_node][to_node] = flow
    pub links: HashMap<String, HashMap<String, f64>>,
    /// Node volumes: accumulated flow through each node
    pub nodes: HashMap<String, f64>,
}

pub fn assign_demand<'a>(
    all_links: &'a [Link],
    all_stops: &HashSet<String>,
    optimal_strategy: &Strategy<'a>,
    trips: &HashMap<String, HashMap<String, f64>>,
    destination: &str,
) -> Volumes {
    // Work on a copy so the caller's ASet order is preserved.
    let mut sorted: Vec<&'a Link> = optimal_strategy.a_set.clone();

    // Sort attractive links by decreasing (u_j + c_a).
    // Tie-break by decreasing u[FromNode] so upstream nodes are loaded first.
    let labels = &optimal_strategy.labels;
    sorted.sort_by(|a, b| {
        let ai = labels.get(&a.to_node).copied().unwrap_or(0.0) + a.travel_cost;
        let aj = labels.get(&b.to_node).copied().unwrap_or(0.0) + b.travel_cost;
        if ai != aj {
            return aj.partial_cmp(&ai).unwrap_or(Ordering::Equal);
        }
        let from_a = labels.get(&a.from_node).copied().unwrap_or(0.0);
        let from_b = labels.get(&b.from_node).copied().unwrap_or(0.0);
        from_b.partial_cmp(&from_a).unwrap_or(Ordering::Equal)
    });

    let mut node_volumes: HashMap<String, f64> = HashMap::with_capacity(all_stops.len());
    for i in all_stops {
        node_volumes.insert(i.clone(), 0.0);
    }
    for (origin, dests) in trips {
        if let Some(&trips_num) = dests.get(destination) {
            node_volumes.insert(origin.clone(), trips_num);
            *node_volumes.entry(destination.to_string()).or_insert(0.0) += trips_num;
        }
    }
    // Destination absorbs flow: negate so arrivals cancel it to zero.
    *node_volumes.entry(destination.to_string()).or_insert(0.0) *= -1.0;

    let mut v: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for a in all_links {
        v.entry(a.from_node.clone())
            .or_default()
            .insert(a.to_node.clone(), 0.0);
    }

    for a in sorted.iter() {
        let mut freq = INFINITE_FREQUENCY;
        if a.headway > 0.0 {
            freq = 1.0 / a.headway;
        }
        let f_i = optimal_strategy.freqs.get(&a.from_node).copied().unwrap_or(0.0);
        let node_volume = node_volumes.get(&a.from_node).copied().unwrap_or(0.0);
        let va = (freq / f_i) * node_volume;
        if verbose() {
            let to_volume = node_volumes.get(&a.to_node).copied().unwrap_or(0.0);
            println!(
                "Assigning demand for link: ({}, {}) \\\\ ",
                a.from_node, a.to_node
            );
            println!(
                "\\quad $v_{{({}, {})}} = \\frac{{{}}}{{{}}}{} = {}$ \\\\ ",
                a.from_node, a.to_node, freq, f_i, node_volume, va
            );
            println!(
                "\\quad $V_{{{}}} = V_{{{}}} + v_{{({}, {}) = {} + {} = {}}}$ \\\\ ",
                a.to_node,
                a.to_node,
                a.from_node,
                a.to_node,
                to_volume,
                va,
                to_volume + va
            );
        }
        v.entry(a.from_node.clone())
            .or_default()
            .insert(a.to_node.clone(), va);
        *node_volumes.entry(a.to_node.clone()).or_insert(0.0) += va;
    }
    if verbose() {
        println!("Final node volumes: \\\\");
        for (k, volume) in &node_volumes {
            println!("\\quad $V_{{{}}} = {}$ \\\\ ", k, volume);
        }
    }

    Volumes {
        links: v,
        nodes: node_volumes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assign_demand() {
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
        let od_matrix: HashMap<String, HashMap<String, f64>> = HashMap::from([(
            "A".to_string(),
            HashMap::from([("B".to_string(), 1.0)]),
        )]);
        let optimal_strategy = Strategy {
            labels: HashMap::from([
                ("A".to_string(), 27.75),
                ("X".to_string(), 19.071428571428573),
                ("X2".to_string(), 17.5),
                ("Y".to_string(), 11.5),
                ("Y3".to_string(), 4.0),
                ("B".to_string(), 0.0),
            ]),
            freqs: HashMap::from([
                ("A".to_string(), 1.0 / 3.0),
                ("X".to_string(), 7.0 / 30.0),
                ("X2".to_string(), INFINITE_FREQUENCY),
                ("Y".to_string(), 0.4),
                ("Y3".to_string(), INFINITE_FREQUENCY),
                ("B".to_string(), 0.0),
            ]),
            a_set: vec![
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
            ],
        };
        let volumes = assign_demand(
            &all_links,
            &all_nodes,
            &optimal_strategy,
            &od_matrix,
            destination_node,
        );

        let correct_links: HashMap<&str, HashMap<&str, f64>> = HashMap::from([
            ("A", HashMap::from([("B", 0.5), ("X2", 0.5)])),
            ("X2", HashMap::from([("X", 0.0), ("Y", 0.5)])),
            ("X", HashMap::from([("X2", 0.0), ("Y3", 0.0)])),
            (
                "Y",
                HashMap::from([("Y3", 1.0 / 12.0), ("B", 5.0 / 12.0)]),
            ),
            ("Y3", HashMap::from([("Y", 0.0), ("B", 1.0 / 12.0)])),
        ]);
        let correct_nodes: HashMap<&str, f64> = HashMap::from([
            ("A", 1.0),
            ("X2", 0.5),
            ("X", 0.0),
            ("Y3", 1.0 / 12.0),
            ("Y", 0.5),
            ("B", 0.0),
        ]);

        assert_eq!(
            volumes.links.len(),
            correct_links.len(),
            "Incorrect number of links in volumes data"
        );
        assert_eq!(
            volumes.nodes.len(),
            correct_nodes.len(),
            "Incorrect number of nodes in volumes data"
        );

        const EPS: f64 = 1e-9;
        for (from_node, to_map) in &volumes.links {
            assert!(
                correct_links.contains_key(from_node.as_str()),
                "No 'FromNode' in correct volumes data"
            );
            for (to_node, volume) in to_map {
                assert!(
                    correct_links[from_node.as_str()].contains_key(to_node.as_str()),
                    "No 'ToNode' in correct volumes data"
                );
                let want = correct_links[from_node.as_str()][to_node.as_str()];
                assert!(
                    (volume - want).abs() <= EPS,
                    "Incorrect volume in link ({}, {}): got {}, want {}",
                    from_node,
                    to_node,
                    volume,
                    want
                );
            }
        }
        for (node, node_volume) in &volumes.nodes {
            assert!(
                correct_nodes.contains_key(node.as_str()),
                "No node in correct volumes data"
            );
            let want = correct_nodes[node.as_str()];
            assert!(
                (node_volume - want).abs() <= EPS,
                "Incorrect volume in node {}: got {}, want {}",
                node,
                node_volume,
                want
            );
        }
    }
}
