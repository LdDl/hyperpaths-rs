use std::collections::{HashMap, HashSet};

use hyperpaths_rs::{Link, compute_sf};

fn main() {
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
    let od_matrix: HashMap<String, HashMap<String, f64>> =
        HashMap::from([("A".to_string(), HashMap::from([("B".to_string(), 1.0)]))]);
    let res = compute_sf(&all_links, &all_nodes, destination_node, &od_matrix);
    println!("Optimal strategy:");
    println!("\tNode labels:");
    for (node_id, node_label) in &res.strategy.labels {
        println!("\t\tu_{{i}} = {}: {:.6}", node_id, node_label);
    }
    println!("\tNodes probablities:");
    for (node_id, freq) in &res.strategy.freqs {
        println!("\t\tf_{{i}} = {}: {:.6}", node_id, freq);
    }
    println!("\tAttractive links set:");
    for link in &res.strategy.a_set {
        println!("\t\t a = (i, j) = ({}, {})", link.from_node, link.to_node);
    }
    println!("Volumes:");
    println!("\tLinks volumes:");
    for (from_node, to_map) in &res.volumes.links {
        for (to_node, volume) in to_map {
            println!(
                "\t\tv_{{i, j}} = ({}, {}): {:.6}",
                from_node, to_node, volume
            );
        }
    }
    println!("\tNodes volumes:");
    for (node_id, volume) in &res.volumes.nodes {
        println!("\t\tv_{{i}} = {}: {:.6}", node_id, volume);
    }
}
