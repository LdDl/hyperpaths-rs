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
    /// f_{i} - combined frequency of attractive links at node i.
    /// `f64::INFINITY` marks a node whose basket is a single no-wait link.
    pub freqs: HashMap<String, f64>,
    /// \overline{A} - attractive links forming the hyperpath
    pub a_set: Vec<&'a Link>,
}

/// The waiting-time constant of the Spiess-Florian expected travel time:
///   u_i = (1 + sum(f_a * (c_a + u_j))) / f_i
/// When the first attractive link arrives at a node the sum is empty and
/// the numerator starts from this constant.
pub(crate) const ALPHA: f64 = 1.0;

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

    // Integer arena: map every node name to a dense index once, so the hot
    // loops below index slices instead of hashing strings on every access.
    // all_stops is interned first (indices [0, n_stops)) so the returned
    // labels/freqs keep exactly the all_stops key set; any link endpoint
    // outside all_stops (out of contract) is appended after and left out.
    let mut n_id: HashMap<&str, usize> = HashMap::with_capacity(all_stops.len());
    let mut n_name: Vec<&str> = Vec::with_capacity(all_stops.len());
    macro_rules! intern {
        ($s:expr) => {{
            let s: &str = $s;
            match n_id.get(s) {
                Some(&id) => id,
                None => {
                    let id = n_name.len();
                    n_id.insert(s, id);
                    n_name.push(s);
                    id
                }
            }
        }};
    }

    for stop in all_stops {
        intern!(stop.as_str());
    }
    let n_stops = n_name.len();

    let m = all_links.len();
    let mut l_from = vec![0usize; m];
    let mut l_to = vec![0usize; m];
    let mut l_cost = vec![0.0f64; m];
    let mut l_head = vec![0.0f64; m];
    for (k, link) in all_links.iter().enumerate() {
        l_from[k] = intern!(link.from_node.as_str());
        l_to[k] = intern!(link.to_node.as_str());
        l_cost[k] = link.travel_cost;
        l_head[k] = link.headway;
    }
    let dest_id = intern!(destination);
    let n = n_name.len();

    // u_i and f_i as dense slices instead of HashMap<String, f64>.
    let mut u = vec![f64::INFINITY; n];
    let mut f = vec![0.0f64; n];
    u[dest_id] = 0.0;
    if verbose() {
        for (id, name) in n_name.iter().enumerate() {
            println!("$f_{{{}}} = 0$ \\\\ ", name);
            if id == dest_id {
                println!("$u_{{{}}} = 0$ \\\\ ", name);
            } else {
                println!("$u_{{{}}} = Infinity$ \\\\ ", name);
            }
        }
    }

    // overline_a holds accepted link indices in acceptance order; a no-wait
    // link replaces a node's whole basket, and replaced slots become None and
    // are compacted at the end. a_set_idx[node] are that node's positions.
    let mut overline_a: Vec<Option<usize>> = Vec::with_capacity(m / 2);
    let mut a_set_idx: Vec<Vec<usize>> = vec![Vec::new(); n];

    // Adjacency by head node: the link indices whose to-node == node, so that
    // when u[node] improves exactly those incoming links are re-keyed.
    let mut adj_by_to: Vec<Vec<usize>> = vec![Vec::new(); n];
    for k in 0..m {
        adj_by_to[l_to[k]].push(k);
    }

    // One priority-queue entry per link, pushed in link order so the entry id
    // equals the link index; this lets the update step reach a link's entry
    // directly, with no scan.
    let mut pq = PriorityQueue::with_capacity(m);
    for k in 0..m {
        pq.push(k, u[l_to[k]] + l_cost[k]);
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
        let k = pq.link(entry_id);
        let i = l_from[k];
        let j = l_to[k];
        let sum_uc = u[j] + l_cost[k];

        /* 1.3 Update node label */
        if verbose() {
            println!("Process: $a = (i, j) = ({}, {})$, \\\\ ", n_name[i], n_name[j]);
        }
        // A node already served by a no-wait link is final: the no-wait
        // link absorbs all flow (its share f_a/f_i is 1 in the limit),
        // so no other link may enter the basket
        if f[i].is_infinite() {
            continue;
        }
        // Strict improvement test: a link is accepted only if it
        // strictly improves the label. Step 1.3 of Spiess & Florian
        // (1989) prints the nonstrict u_i >= u_j + c_a, but the two
        // rules differ only at exact equality, where the update is a
        // no-op (the combination formula returns u_i unchanged; for
        // f_a = inf the basket is replaced at the same value): labels,
        // expected travel times and every number published in the
        // paper are identical either way. The strict form is what
        // part 2 needs. Step 2.2 loads links "in reverse topological
        // order (decreasing u_j + c_a)" (p. 94) and Proposition 4
        // claims flow conservation "by construction" - both presume an
        // acyclic strategy, which the nonstrict rule does not
        // guarantee: in an expanded route graph a boarding link (cost
        // 0) into a route node whose label came from its own alighting
        // link (cost 0) has key exactly u_i, so >= admits a zero-cost
        // stop -> node -> stop cycle and the one-pass loading strands
        // the volume entering it (see
        // test_board_alight_loop_conservation). Rejecting at equality
        // keeps the strategy acyclic and stays optimal: for the
        // rejected link mu_a = 0 satisfies dual feasibility (20) as an
        // equality and complementary slackness (24) holds since
        // v_a = 0, a degenerate optimum. The prose of p. 94 ("if this
        // time is smaller than u_i, link a is included") describes
        // exactly this strict rule. All step, equation and page
        // references above are to the original paper, not to the
        // spiess_floarian.tex excerpt in this repository.
        if u[i] <= sum_uc {
            continue;
        }
        if verbose() {
            println!(
                "\\quad $u_i \\leq u_j + c_a : {} \\leq {}$ - FALSE \\\\ ",
                u[i], sum_uc
            );
        }
        if l_head[k] <= 0.0 {
            // No-wait link (infinite frequency): the modified step 1.3
            // given by the paper on p. 96 - the exact limit of the label
            // update formula as f_a -> inf. The link replaces the whole
            // attractive basket:
            //   u_i := u_j + c_a, f_i := inf, A_i := {a}
            u[i] = sum_uc;
            f[i] = f64::INFINITY;
            for &idx in &a_set_idx[i] {
                overline_a[idx] = None;
            }
            a_set_idx[i].clear();
            overline_a.push(Some(k));
            a_set_idx[i].push(overline_a.len() - 1);
            if verbose() {
                println!(
                    "\\quad no-wait link: $u_i = u_j + c_a = {}$, $f_i = \\infty$, basket replaced by $({}, {})$ \\\\ ",
                    sum_uc, n_name[i], n_name[j]
                );
            }
        } else {
            let freq = 1.0 / l_head[k];
            if verbose() {
                println!("\\quad $f_a = {}$ \\\\ ", freq);
                println!("\\quad $u_j + c_a = {}$ \\\\ ", sum_uc);
                println!("\\quad $u_i = {}$ \\\\ ", u[i]);
            }
            let new_u = if f[i] == 0.0 {
                // First link in the basket: u_i = (1 + f_a*(u_j+c_a)) / f_a
                (ALPHA + freq * sum_uc) / freq
            } else {
                (f[i] * u[i] + freq * sum_uc) / (f[i] + freq)
            };
            u[i] = new_u;
            f[i] += freq;
            overline_a.push(Some(k));
            a_set_idx[i].push(overline_a.len() - 1);
            if verbose() {
                println!(
                    "\\quad$u_i = \\frac{{f_i * u_i + f_a * (u_j + c_a)}}{{f_i + f_a}} = {}$, $f_i = {}$ \\\\ ",
                    new_u, f[i]
                );
                println!(
                    "\\quad $\\overline{{A}} = \\overline{{A}} \\cup {{({}, {})}}$ \\\\ ",
                    n_name[i], n_name[j]
                );
            }
        }

        // u[i] improved: re-key exactly the links entering i. The entry id
        // equals the link index, so update reaches each directly.
        for &kk in &adj_by_to[i] {
            pq.update(kk, u[i] + l_cost[kk]);
        }
        if verbose() {
            println!("Node labels: \\\\");
            for (id, name) in n_name.iter().enumerate() {
                println!("${} -> (u_i, f_i) = ({}, {})$ \\\\ ", name, u[id], f[id]);
            }
        }
    }

    // Compact the attractive set: drop entries replaced by no-wait links.
    // The append order is preserved, i.e. non-decreasing u_j + c_a.
    let a_set: Vec<&'a Link> = overline_a
        .into_iter()
        .flatten()
        .map(|k| &all_links[k])
        .collect();

    // Translate the arena labels/freqs back to the public string-keyed maps,
    // for the all_stops key set only.
    let mut labels: HashMap<String, f64> = HashMap::with_capacity(n_stops);
    let mut freqs: HashMap<String, f64> = HashMap::with_capacity(n_stops);
    for id in 0..n_stops {
        labels.insert(n_name[id].to_string(), u[id]);
        freqs.insert(n_name[id].to_string(), f[id]);
    }

    Strategy {
        labels,
        freqs,
        a_set,
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

        // With exact no-wait handling the labels match the paper exactly:
        // no big-M artifacts like 4.000000000000001
        let expected_labels: HashMap<&str, f64> = HashMap::from([
            ("A", 27.75),
            ("X", 19.071428571428573),
            ("X2", 17.5),
            ("Y", 11.5),
            ("Y3", 4.0),
            ("B", 0.0),
        ]);
        // +Inf marks nodes whose basket is a single no-wait link
        let expected_freqs: HashMap<&str, f64> = HashMap::from([
            ("A", 1.0 / 3.0),
            ("X", 7.0 / 30.0),
            ("X2", f64::INFINITY),
            ("Y", 0.4),
            ("Y3", f64::INFINITY),
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
            if want.is_infinite() {
                assert!(
                    v.is_infinite() && *v > 0.0,
                    "Frequency for node {} must be +Inf, got {}",
                    k,
                    v
                );
            } else {
                assert!(
                    (v - want).abs() <= EPS,
                    "Incorrect frequency value for node {}: got {}, want {}",
                    k,
                    v,
                    want
                );
            }
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

    #[test]
    fn test_no_wait_replaces_basket() {
        // A boarding link enters the basket of I first (key 4), then a
        // cheaper no-wait chain I->W->D (key 5 < current u_I = 10) must
        // replace it entirely: exact label, infinite frequency, single link.
        let all_nodes: HashSet<String> = ["I", "W", "D"].iter().map(|s| s.to_string()).collect();
        let all_links = vec![
            // boarding link, key u_D + 4 = 4, accepted first: u_I = 6 + 4 = 10
            Link::new("I", "D", "Bus", 4.0, 6.0),
            // no-wait walk, key u_W + 3 = 5, replaces the basket: u_I = 5
            Link::new("I", "W", "Walk", 3.0, 0.0),
            // no-wait walk, key 2
            Link::new("W", "D", "Walk", 2.0, 0.0),
        ];
        let ops = find_optimal_strategy(&all_links, &all_nodes, "D");

        assert!((ops.labels["I"] - 5.0).abs() <= 1e-12);
        assert!((ops.labels["W"] - 2.0).abs() <= 1e-12);
        assert!(ops.freqs["I"].is_infinite());
        assert!(ops.freqs["W"].is_infinite());

        // The replaced boarding link I->D must not remain attractive
        assert_eq!(
            ops.a_set.len(),
            2,
            "basket of I must hold only the no-wait link"
        );
        for link in &ops.a_set {
            assert_eq!(
                link.headway, 0.0,
                "only no-wait links expected in the attractive set"
            );
        }
    }
}
