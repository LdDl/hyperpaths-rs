use crate::spiess_floarian::compute_sf;
use crate::testutil::gen_grid_network;

#[test]
fn test_golden_grid() {
    // Locks the exact result of compute_sf on a fixed 4x4 synthetic grid. The
    // anchor values were captured from the reference implementation; the
    // integer-arena rewrite must reproduce them exactly, so this guards the
    // optimization against altering results.
    let (links, nodes, dest, od) = gen_grid_network(4, 4, 6.0, 3.0);
    let res = compute_sf(&links, &nodes, &dest, &od);

    const EPS: f64 = 1e-9;
    assert!((res.strategy.labels["s_0_0"] - 27.0).abs() < EPS);
    assert!((res.strategy.labels["s_2_1"] - 18.0).abs() < EPS);
    assert!((res.strategy.freqs["s_0_0"] - 1.0 / 3.0).abs() < EPS);
    assert_eq!(res.strategy.a_set.len(), 56);

    let mut total = 0.0;
    for m in res.volumes.links.values() {
        for v in m.values() {
            total += v;
        }
    }
    assert!((total - 96.0).abs() < EPS, "total volume {}", total);
}
