use rmatching::Matching;

#[test]
fn debug_chain_5events_short() {
    // 6 nodes, boundary at 0, 5 events at 1,2,3,4,5
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[], -1.0);
    for i in 0..5 {
        m.add_edge(i, i + 1, 1.0, &[], -1.0);
    }
    let c = m.decode(&[0, 1, 1, 1, 1, 1]);
    eprintln!("5-event short: {:?}", c);
}
