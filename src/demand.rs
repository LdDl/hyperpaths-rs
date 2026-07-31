use std::collections::{HashMap, HashSet};

use crate::hyperpath::{verbose, Strategy};
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
    // The attractive set is built in acceptance order, which is
    // non-decreasing u_j + c_a (heap pops), so its reverse is exactly the
    // paper's decreasing loading order - no sorting needed, as the paper
    // notes on p. 97: the processing order of step 2.2 "is the inverse of
    // the order used in part 1 of the algorithm". At zero-cost ties
    // (no-wait chains produce exactly equal keys) reverse acceptance
    // order also guarantees that a node's inflow links are loaded before
    // its outflow links: a link (i, j) is accepted before the links into i
    // are updated and popped.
    let sorted = &optimal_strategy.a_set;

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

    for a in sorted.iter().rev() {
        let f_i = optimal_strategy.freqs.get(&a.from_node).copied().unwrap_or(0.0);
        let node_volume = node_volumes.get(&a.from_node).copied().unwrap_or(0.0);
        let va = if f_i.is_infinite() {
			// A no-wait basket holds exactly one link (the one that replaced it);
            // per the paper's modified step 2.2 (p. 96) the link takes
			// the whole node volume: v_a := V_i
            node_volume
        } else {
            // A finite basket holds only boarding links (headway > 0)
            let freq = 1.0 / a.headway;
            (freq / f_i) * node_volume
        };
        if verbose() {
            let to_volume = node_volumes.get(&a.to_node).copied().unwrap_or(0.0);
            println!(
                "Assigning demand for link: ({}, {}) \\\\ ",
                a.from_node, a.to_node
            );
            println!(
                "\\quad $v_{{({}, {})}} = {}$, $V_{{{}}} = {} + {}$ \\\\ ",
                a.from_node, a.to_node, va, a.to_node, to_volume, va
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
    fn test_zero_cost_chain_loading() {
        // Regression for the loading order at exact zero-cost ties: the
        // alighting link (B1 -> S2) and the walking link (S2 -> S3) both have
        // key u_j + c_a = 4 and equal tail labels, so no sort comparator can
        // recover the dependency between them. Reverse acceptance order must
        // load the inflow of S2 before its outflow.
        use crate::hyperpath::find_optimal_strategy;

        let all_nodes: HashSet<String> = ["S1", "B0", "B1", "S2", "S3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all_links = vec![
            // boarding: wait for the bus (headway 5), no riding yet
            Link::new("S1", "B0", "Bus", 0.0, 5.0),
            // on-board segment
            Link::new("B0", "B1", "Bus", 10.0, 0.0),
            // alighting, key u_S2 + 0 = 4
            Link::new("B1", "S2", "Bus", 0.0, 0.0),
            // walking to the destination, key u_S3 + 4 = 4
            Link::new("S2", "S3", "Walk", 4.0, 0.0),
        ];
        let ops = find_optimal_strategy(&all_links, &all_nodes, "S3");
        // 5 wait + 10 ride + 0 alight + 4 walk
        assert!((ops.labels["S1"] - 19.0).abs() <= 1e-12);

        let trips: HashMap<String, HashMap<String, f64>> = HashMap::from([(
            "S1".to_string(),
            HashMap::from([("S3".to_string(), 100.0)]),
        )]);
        let volumes = assign_demand(&all_links, &all_nodes, &ops, &trips, "S3");
        assert!((volumes.links["S1"]["B0"] - 100.0).abs() <= 1e-12);
        assert!((volumes.links["B0"]["B1"] - 100.0).abs() <= 1e-12);
        assert!((volumes.links["B1"]["S2"] - 100.0).abs() <= 1e-12);
        assert!((volumes.links["S2"]["S3"] - 100.0).abs() <= 1e-12);
    }

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
                ("X2".to_string(), f64::INFINITY),
                ("Y".to_string(), 0.4),
                ("Y3".to_string(), f64::INFINITY),
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
