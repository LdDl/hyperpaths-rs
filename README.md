# Hyperpath routing in Rust

Just implementation of [Spiess, H. and Florian, M. (1989) "Optimal strategies: A new assignment model for transit networks"](https://doi.org/10.1016/0191-2615(89)90034-9) in Rust

**Note:** this is a Rust port of the [go-hyperpaths](https://github.com/LdDl/go-hyperpaths) project.

## Algorithm

Here is copy of algorithm in MathJax (for the LaTeX see [spiess_floarian.tex](./spiess_floarian.tex)):

### Part 1: Find optimal strategy

1. **Initialization**
   - Set $u_r = 0$ for destination node
   - Set $u_i = \infty$ for all other nodes
   - Set $f_i = 0$ for all nodes
   - Initialize empty attractive set $\overline{A}$

2. **Label Setting**
   - For each link $a = (i,j)$ with minimum $u_j + c_a$
   - If $u_i \geq u_j + c_a$:
     * Update node label: $$u_i = \frac{f_i \cdot u_i + f_a \cdot (u_j + c_a)}{f_i + f_a}$$
     * Update frequency: $$f_i = f_i + f_a$$
     * Add to attractive set: $$\overline{A} = \overline{A} \cup \{a\}$$
    
### Part 2: Assign demand according to optimal strategy

1. **Initialization**
   - Set $V_i = g_i$ for all nodes

2. **Loading**
   - Process links in decreasing order of $u_j + c_a$
   - For attractive links $a \in \overline{A}$:
     * Calculate volume: $$v_a = \frac{f_a}{f_i}V_i$$
     * Update node volume: $$V_j = V_j + v_a$$

### Infinite frequencies (no-wait links)

Links with `headway = 0` (walking, alighting, on-board) have infinite
frequency. Instead of a big-M constant, the implementation follows the
modified version of the algorithm given by the paper itself (p. 96):
such a link replaces the whole attractive set of its tail node,
$u_i := u_j + c_a$, $f_i := \infty$, $\overline{A}_i := \{a\}$, and during
loading it takes the entire node volume ($v_a := V_i$). The paper's own
worked example uses this modified version, so the labels match it
exactly (e.g. $u_{Y3} = 4$, not $4 + \varepsilon$).

The loading phase needs no sorting, as the paper notes on p. 97: "no
additional computations are needed to establish the order in which the
links are processed, since it is the inverse of the order used in part 1
of the algorithm". The attractive set is built in acceptance order
(non-decreasing $u_j + c_a$), so reverse iteration is exactly the
required decreasing order (Table 3 of the paper), and at zero-cost ties
it loads a node's inflow links before its outflow links by construction.

## How to use

* Get the package:
   ```shell
   cargo add hyperpaths-rs
   ```

* Code (you can find it in [examples/paper](./examples/paper), run it with `cargo run --example paper`)
   ```rust
   use std::collections::{HashMap, HashSet};

   use hyperpaths_rs::{compute_sf, Link};

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
       let od_matrix: HashMap<String, HashMap<String, f64>> = HashMap::from([(
           "A".to_string(),
           HashMap::from([("B".to_string(), 1.0)]),
       )]);
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
               println!("\t\tv_{{i, j}} = ({}, {}): {:.6}", from_node, to_node, volume);
           }
       }
       println!("\tNodes volumes:");
       for (node_id, volume) in &res.volumes.nodes {
           println!("\t\tv_{{i}} = {}: {:.6}", node_id, volume);
       }
   }
   ```

## References
Spiess, H. and Florian, M. (1989) "Optimal strategies: A new assignment model for transit networks". Transportation Research Part B: Methodological, 23(2), 83-102. Available in: https://doi.org/10.1016/0191-2615(89)90034-9
