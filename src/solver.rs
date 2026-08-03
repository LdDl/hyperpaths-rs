//! Reusable arena solver: build an immutable [`Graph`] once, then assign many
//! destinations through a [`Workspace`] with no per-destination allocation.
//!
//! Splitting the immutable graph from the mutable per-solve buffers is what
//! makes the multi-destination / multi-request case cheap: the string interning
//! and adjacency are built a single time, and each destination is then assigned
//! by reusing a `Workspace`.
//!
//! Concurrency is enforced by the type system: a `&Graph` is shared and
//! read-only (so concurrent assignments are safe), while a `Workspace` is
//! `&mut` and therefore exclusive - the borrow checker will not let two threads
//! mutate one. Build the graph once and give each thread its own workspace:
//!
//! ```
//! use std::collections::{HashMap, HashSet};
//! use hyperpaths_rs::{Graph, Link};
//!
//! let links = vec![Link::new("A", "B", "L", 10.0, 6.0)];
//! let stops: HashSet<String> = ["A", "B"].iter().map(|s| s.to_string()).collect();
//! let graph = Graph::new(&links, &stops);
//!
//! std::thread::scope(|s| {
//!     for _ in 0..4 {
//!         let g = &graph; // shared, immutable
//!         s.spawn(move || {
//!             let mut w = g.new_workspace(); // one per thread
//!             let mut demand = vec![0.0; g.num_nodes()];
//!             demand[g.node_index("A").unwrap()] = 1.0;
//!             let res = w.assign(g.node_index("B").unwrap(), &demand);
//!             let _ = res.labels;
//!         });
//!     }
//! });
//! ```

use std::collections::{HashMap, HashSet};

use crate::hyperpath::ALPHA;
use crate::hyperpath_queue::PriorityQueue;
use crate::transit_network::Link;

fn intern(name: &str, n_id: &mut HashMap<String, usize>, n_name: &mut Vec<String>) -> usize {
    if let Some(&id) = n_id.get(name) {
        return id;
    }
    let id = n_name.len();
    n_id.insert(name.to_string(), id);
    n_name.push(name.to_string());
    id
}

/// Graph is an immutable, interned transit network (integer arena). Build it
/// once with [`Graph::new`] and share it across threads; it is read-only.
pub struct Graph {
    n_name: Vec<String>,
    n_id: HashMap<String, usize>,
    n: usize,
    m: usize,
    from: Vec<usize>,
    to: Vec<usize>,
    cost: Vec<f64>,
    head: Vec<f64>,
    adj_by_to: Vec<Vec<usize>>,
}

impl Graph {
    /// Interns the network once. `all_stops` is interned first, so node indices
    /// `[0, all_stops.len())` are the stop nodes; any link endpoint outside
    /// `all_stops` (out of contract) is appended after.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use hyperpaths_rs::{Graph, Link};
    ///
    /// // One line A -> B: 6-minute headway, 10-minute ride.
    /// let links = vec![Link::new("A", "B", "L1", 10.0, 6.0)];
    /// let stops: HashSet<String> = ["A", "B"].iter().map(|s| s.to_string()).collect();
    ///
    /// let graph = Graph::new(&links, &stops);      // once; immutable, shareable
    /// let mut w = graph.new_workspace();            // reusable buffers
    ///
    /// let a = graph.node_index("A").unwrap();
    /// let b = graph.node_index("B").unwrap();
    /// let mut demand = vec![0.0; graph.num_nodes()];
    /// demand[a] = 1.0; // one trip from A to B
    ///
    /// let res = w.assign(b, &demand);
    /// // Expected time A -> B: 6 min wait + 10 min ride.
    /// assert!((res.labels[a] - 16.0).abs() < 1e-9);
    /// assert!((res.link_vol[0] - 1.0).abs() < 1e-9);
    /// ```
    pub fn new(all_links: &[Link], all_stops: &HashSet<String>) -> Graph {
        let mut n_id: HashMap<String, usize> = HashMap::with_capacity(all_stops.len());
        let mut n_name: Vec<String> = Vec::with_capacity(all_stops.len());
        for stop in all_stops {
            intern(stop.as_str(), &mut n_id, &mut n_name);
        }
        let m = all_links.len();
        let mut from = vec![0usize; m];
        let mut to = vec![0usize; m];
        let mut cost = vec![0.0f64; m];
        let mut head = vec![0.0f64; m];
        for (k, link) in all_links.iter().enumerate() {
            from[k] = intern(link.from_node.as_str(), &mut n_id, &mut n_name);
            to[k] = intern(link.to_node.as_str(), &mut n_id, &mut n_name);
            cost[k] = link.travel_cost;
            head[k] = link.headway;
        }
        let n = n_name.len();
        let mut adj_by_to: Vec<Vec<usize>> = vec![Vec::new(); n];
        for k in 0..m {
            adj_by_to[to[k]].push(k);
        }
        Graph {
            n_name,
            n_id,
            n,
            m,
            from,
            to,
            cost,
            head,
            adj_by_to,
        }
    }

    /// Number of nodes (size demand buffers with this).
    pub fn num_nodes(&self) -> usize {
        self.n
    }

    /// Number of links.
    pub fn num_links(&self) -> usize {
        self.m
    }

    /// Arena index of a node name, or `None` if unknown.
    pub fn node_index(&self, name: &str) -> Option<usize> {
        self.n_id.get(name).copied()
    }

    /// Name of an arena node index.
    pub fn node_name(&self, id: usize) -> &str {
        &self.n_name[id]
    }

    /// Allocates the working buffers for this graph once; reuse the workspace
    /// across destinations and requests.
    pub fn new_workspace(&self) -> Workspace<'_> {
        Workspace {
            g: self,
            u: vec![0.0; self.n],
            f: vec![0.0; self.n],
            pq: PriorityQueue::with_capacity(self.m),
            overline_a: Vec::with_capacity(self.m / 2),
            a_set_idx: vec![Vec::new(); self.n],
            a_set: Vec::with_capacity(self.m / 2),
            link_vol: vec![0.0; self.m],
            node_vol: vec![0.0; self.n],
            cols: vec![Vec::new(); self.n],
        }
    }
}

/// DestResult is one destination's assignment in arena (integer) indexing. Its
/// slices borrow the [`Workspace`] and are reused on the next assign, so copy
/// out anything that must outlive it (the borrow checker enforces this).
pub struct DestResult<'w> {
    pub dest_id: usize,
    /// node -> expected time to destination (u_i)
    pub labels: &'w [f64],
    /// node -> combined attractive frequency (f_i); +Inf = no-wait
    pub freqs: &'w [f64],
    /// accepted link indices, in acceptance order
    pub a_set: &'w [usize],
    /// link -> assigned volume
    pub link_vol: &'w [f64],
    /// node -> accumulated volume
    pub node_vol: &'w [f64],
}

/// Workspace holds the reusable per-solve buffers for one [`Graph`]. Create one
/// per thread (it borrows the graph immutably); it is not shareable while in
/// use because its methods take `&mut self`.
pub struct Workspace<'g> {
    g: &'g Graph,
    u: Vec<f64>,
    f: Vec<f64>,
    pq: PriorityQueue,
    overline_a: Vec<Option<usize>>,
    a_set_idx: Vec<Vec<usize>>,
    a_set: Vec<usize>,
    link_vol: Vec<f64>,
    node_vol: Vec<f64>,
    cols: Vec<Vec<(usize, f64)>>,
}

impl Workspace<'_> {
    /// Spiess-Florian phase 1 for `dest_id` into u, f and a_set, reusing the
    /// buffers. Identical algorithm to `find_optimal_strategy`, arena-indexed.
    fn find_strategy(&mut self, dest_id: usize) {
        let g = self.g;
        for id in 0..g.n {
            self.f[id] = 0.0;
            self.u[id] = if id == dest_id { 0.0 } else { f64::INFINITY };
            self.a_set_idx[id].clear();
        }
        self.overline_a.clear();

        self.pq.clear();
        for k in 0..g.m {
            self.pq.push(k, self.u[g.to[k]] + g.cost[k]);
        }
        self.pq.init();

        while self.pq.len() > 0 {
            let entry_id = match self.pq.pop() {
                Some(id) => id,
                None => break,
            };
            let priority = self.pq.priority(entry_id);
            if priority.is_infinite() && priority > 0.0 {
                break;
            }
            let k = self.pq.link(entry_id);
            let i = g.from[k];
            let j = g.to[k];
            let sum_uc = self.u[j] + g.cost[k];

            if self.f[i].is_infinite() {
                continue;
            }
            if self.u[i] <= sum_uc {
                continue;
            }
            if g.head[k] <= 0.0 {
                self.u[i] = sum_uc;
                self.f[i] = f64::INFINITY;
                for &idx in &self.a_set_idx[i] {
                    self.overline_a[idx] = None;
                }
                self.a_set_idx[i].clear();
                self.overline_a.push(Some(k));
                self.a_set_idx[i].push(self.overline_a.len() - 1);
            } else {
                let freq = 1.0 / g.head[k];
                let new_u = if self.f[i] == 0.0 {
                    (ALPHA + freq * sum_uc) / freq
                } else {
                    (self.f[i] * self.u[i] + freq * sum_uc) / (self.f[i] + freq)
                };
                self.u[i] = new_u;
                self.f[i] += freq;
                self.overline_a.push(Some(k));
                self.a_set_idx[i].push(self.overline_a.len() - 1);
            }

            for &kk in &g.adj_by_to[i] {
                self.pq.update(kk, self.u[i] + g.cost[kk]);
            }
        }

        self.a_set.clear();
        for &opt in &self.overline_a {
            if let Some(k) = opt {
                self.a_set.push(k);
            }
        }
    }

    /// Phase 2: with node_vol seeded and a_set ready, load flow into link_vol
    /// (reverse acceptance order, p. 97).
    fn load(&mut self) {
        let g = self.g;
        for k in 0..g.m {
            self.link_vol[k] = 0.0;
        }
        for idx in (0..self.a_set.len()).rev() {
            let k = self.a_set[idx];
            let i = g.from[k];
            let f_i = self.f[i];
            let va = if f_i.is_infinite() {
                self.node_vol[i]
            } else {
                let freq = 1.0 / g.head[k];
                (freq / f_i) * self.node_vol[i]
            };
            self.link_vol[k] = va;
            self.node_vol[g.to[k]] += va;
        }
    }

    /// Runs the full assignment (optimal strategy + demand loading) for one
    /// destination index. `demand` is a per-node slice of trips heading to
    /// `dest_id` (`demand[dest_id]` is ignored). The result borrows the
    /// workspace and is valid until the next assign on it.
    pub fn assign(&mut self, dest_id: usize, demand: &[f64]) -> DestResult<'_> {
        self.find_strategy(dest_id);
        let mut total = 0.0;
        for (id, (nv, &d)) in self.node_vol.iter_mut().zip(demand.iter()).enumerate() {
            if id != dest_id && d != 0.0 {
                *nv = d;
                total += d;
            } else {
                *nv = 0.0;
            }
        }
        self.node_vol[dest_id] = -total;
        self.load();
        DestResult {
            dest_id,
            labels: &self.u,
            freqs: &self.f,
            a_set: &self.a_set,
            link_vol: &self.link_vol,
            node_vol: &self.node_vol,
        }
    }

    /// Assigns every destination present in `od` (an
    /// origin -> destination -> demand matrix) and calls `callback` with the
    /// arena-indexed result for each. It transposes `od` into per-destination
    /// columns once (reusing the workspace buffers), so there are no
    /// per-destination allocations after warm-up. The result is reused between
    /// calls, so copy out anything that must outlive the callback.
    ///
    /// # Example
    ///
    /// Build the graph once, reuse the workspace, and assign a full OD (here one
    /// trip A -> B on the paper network):
    ///
    /// ```
    /// use std::collections::{HashMap, HashSet};
    /// use hyperpaths_rs::{DestResult, Graph, Link};
    ///
    /// let nodes: HashSet<String> = ["A", "X", "X2", "Y", "Y3", "B"]
    ///     .iter()
    ///     .map(|s| s.to_string())
    ///     .collect();
    /// let links = vec![
    ///     Link::new("A", "B", "Line 1", 25.0, 6.0),
    ///     Link::new("A", "X2", "Line 2", 7.0, 6.0),
    ///     Link::new("X2", "X", "Line 2", 0.0, 0.0),
    ///     Link::new("X", "X2", "Line 2", 0.0, 6.0),
    ///     Link::new("X2", "Y", "Line 2", 6.0, 0.0),
    ///     Link::new("Y3", "Y", "Line 3", 0.0, 15.0),
    ///     Link::new("Y", "B", "Line 4", 10.0, 3.0),
    ///     Link::new("X", "Y3", "Line 3", 4.0, 15.0),
    ///     Link::new("Y", "Y3", "Line 3", 0.0, 15.0),
    ///     Link::new("Y3", "B", "Line 3", 4.0, 0.0),
    /// ];
    ///
    /// let graph = Graph::new(&links, &nodes);
    /// let mut w = graph.new_workspace();
    /// let a = graph.node_index("A").unwrap();
    ///
    /// let od = HashMap::from([("A".to_string(), HashMap::from([("B".to_string(), 1.0)]))]);
    ///
    /// let mut a_to_b = 0.0;
    /// w.solve_each(&od, |res: &DestResult| {
    ///     a_to_b = res.labels[a];
    /// });
    /// assert!((a_to_b - 27.75).abs() < 1e-9);
    /// ```
    pub fn solve_each<F: FnMut(&DestResult)>(
        &mut self,
        od: &HashMap<String, HashMap<String, f64>>,
        mut callback: F,
    ) {
        for c in self.cols.iter_mut() {
            c.clear();
        }
        for (origin, row) in od {
            let oid = match self.g.node_index(origin) {
                Some(id) => id,
                None => continue,
            };
            for (dest, &d) in row {
                if d == 0.0 {
                    continue;
                }
                if let Some(did) = self.g.node_index(dest) {
                    self.cols[did].push((oid, d));
                }
            }
        }
        let n = self.g.n;
        for did in 0..n {
            if self.cols[did].is_empty() {
                continue;
            }
            self.find_strategy(did);
            let mut total = 0.0;
            for id in 0..n {
                self.node_vol[id] = 0.0;
            }
            for idx in 0..self.cols[did].len() {
                let (oid, d) = self.cols[did][idx];
                if oid != did {
                    self.node_vol[oid] = d;
                    total += d;
                }
            }
            self.node_vol[did] = -total;
            self.load();
            let res = DestResult {
                dest_id: did,
                labels: &self.u,
                freqs: &self.f,
                a_set: &self.a_set,
                link_vol: &self.link_vol,
                node_vol: &self.node_vol,
            };
            callback(&res);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spiess_floarian::compute_sf;
    use crate::testutil::{gen_grid_network, grid_stops};

    #[test]
    fn test_solver_parity() {
        // The arena Graph/Workspace assignment must produce exactly the same
        // labels and link volumes as the string-keyed compute_sf, on the 4x4
        // grid to the corner destination.
        let (links, nodes, dest, od) = gen_grid_network(4, 4, 6.0, 3.0);
        let reference = compute_sf(&links, &nodes, &dest, &od);

        let g = Graph::new(&links, &nodes);
        let mut w = g.new_workspace();
        let dest_id = g.node_index(&dest).unwrap();

        let mut demand = vec![0.0; g.num_nodes()];
        for (origin, row) in &od {
            if let Some(&v) = row.get(&dest) {
                demand[g.node_index(origin).unwrap()] = v;
            }
        }
        let got = w.assign(dest_id, &demand);

        const EPS: f64 = 1e-9;
        for (name, &want) in &reference.strategy.labels {
            let id = g.node_index(name).unwrap();
            assert!((got.labels[id] - want).abs() < EPS, "label {}", name);
            let wf = reference.strategy.freqs[name];
            if wf.is_infinite() {
                assert!(got.freqs[id].is_infinite());
            } else {
                assert!((got.freqs[id] - wf).abs() < EPS, "freq {}", name);
            }
        }
        for (k, link) in links.iter().enumerate() {
            let want = reference.volumes.links[&link.from_node][&link.to_node];
            assert!(
                (got.link_vol[k] - want).abs() < EPS,
                "linkvol {}->{}",
                link.from_node,
                link.to_node
            );
        }
    }

    #[test]
    fn test_solve_each_parity() {
        // solve_each over a full OD matrix must accumulate exactly the same
        // total link volume as calling compute_sf once per destination, on the
        // 4x4 grid.
        let (links, nodes, _, _) = gen_grid_network(4, 4, 6.0, 3.0);
        let stops = grid_stops(4, 4);

        // full OD: one trip from every stop to every other stop
        let mut od: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for o in &stops {
            let mut row = HashMap::new();
            for d in &stops {
                if d != o {
                    row.insert(d.clone(), 1.0);
                }
            }
            od.insert(o.clone(), row);
        }

        // reference: compute_sf per destination
        let mut want_total = 0.0;
        for dest in &stops {
            let mut col: HashMap<String, HashMap<String, f64>> = HashMap::new();
            for o in &stops {
                if o != dest {
                    col.insert(o.clone(), HashMap::from([(dest.clone(), 1.0)]));
                }
            }
            let res = compute_sf(&links, &nodes, dest, &col);
            for m in res.volumes.links.values() {
                for v in m.values() {
                    want_total += v;
                }
            }
        }

        let g = Graph::new(&links, &nodes);
        let mut w = g.new_workspace();
        let mut got_total = 0.0;
        w.solve_each(&od, |res| {
            for &v in res.link_vol {
                got_total += v;
            }
        });
        assert!((got_total - want_total).abs() < 1e-6, "{} {}", got_total, want_total);
    }

    #[test]
    fn test_concurrent_shared_graph() {
        // Many threads share one immutable Graph and each take their own
        // Workspace. The type system already guarantees no data races; this
        // checks correctness: every thread must reproduce the single-threaded
        // reference total.
        let (links, nodes, _, _) = gen_grid_network(6, 6, 6.0, 3.0);
        let stops = grid_stops(6, 6);
        let mut od: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for o in &stops {
            let mut row = HashMap::new();
            for d in &stops {
                if d != o {
                    row.insert(d.clone(), 1.0);
                }
            }
            od.insert(o.clone(), row);
        }
        let graph = Graph::new(&links, &nodes);

        let mut ref_total = 0.0;
        {
            let mut w = graph.new_workspace();
            w.solve_each(&od, |res| {
                for &v in res.link_vol {
                    ref_total += v;
                }
            });
        }

        std::thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let g = &graph;
                    let od = &od;
                    s.spawn(move || {
                        let mut w = g.new_workspace();
                        let mut total = 0.0;
                        w.solve_each(od, |res| {
                            for &v in res.link_vol {
                                total += v;
                            }
                        });
                        total
                    })
                })
                .collect();
            for h in handles {
                let total = h.join().unwrap();
                assert!((total - ref_total).abs() < 1e-6);
            }
        });
    }
}
