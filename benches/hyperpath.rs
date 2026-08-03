use std::collections::HashMap;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use hyperpaths_rs::testutil::{gen_grid_network, grid_full_od, grid_stops};
use hyperpaths_rs::{Graph, compute_sf, find_optimal_strategy};

fn bench_single_dest(c: &mut Criterion) {
    let sizes = [(8usize, 8usize), (16, 16), (32, 32)];

    let mut fos = c.benchmark_group("find_optimal_strategy");
    for &(rows, cols) in &sizes {
        let (links, nodes, dest, _) = gen_grid_network(rows, cols, 6.0, 3.0);
        let id = BenchmarkId::from_parameter(format!("{}x{}", rows, cols));
        fos.bench_function(id, |b| {
            b.iter(|| black_box(find_optimal_strategy(&links, &nodes, &dest)));
        });
    }
    fos.finish();

    let mut sf = c.benchmark_group("compute_sf");
    for &(rows, cols) in &sizes {
        let (links, nodes, dest, od) = gen_grid_network(rows, cols, 6.0, 3.0);
        let id = BenchmarkId::from_parameter(format!("{}x{}", rows, cols));
        sf.bench_function(id, |b| {
            b.iter(|| black_box(compute_sf(&links, &nodes, &dest, &od)));
        });
    }
    sf.finish();
}

fn bench_full_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_assignment");
    for &(rows, cols) in &[(8usize, 8usize), (12, 12)] {
        let (links, nodes, _, _) = gen_grid_network(rows, cols, 6.0, 3.0);
        let stops = grid_stops(rows, cols);
        let od = grid_full_od(&stops);
        let size = format!("{}x{}", rows, cols);

        // old: compute_sf per destination, rebuilding the arena each time
        group.bench_function(BenchmarkId::new("compute_sf_per_dest", &size), |b| {
            b.iter(|| {
                for dest in &stops {
                    let mut col: HashMap<String, HashMap<String, f64>> = HashMap::new();
                    for o in &stops {
                        if o != dest {
                            col.insert(o.clone(), HashMap::from([(dest.clone(), 1.0)]));
                        }
                    }
                    black_box(compute_sf(&links, &nodes, dest, &col));
                }
            });
        });

        // new: Graph interned once, Workspace reused via solve_each (warm)
        let g = Graph::new(&links, &nodes);
        let mut w = g.new_workspace();
        group.bench_function(BenchmarkId::new("solve_each", &size), |b| {
            b.iter(|| {
                w.solve_each(&od, |res| {
                    black_box(res.link_vol);
                });
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_single_dest, bench_full_assignment);
criterion_main!(benches);
