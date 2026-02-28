use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::sim::tableau::StabilizerState;

#[test]
fn h_then_m_gives_random() {
    let mut st = StabilizerState::new(1);
    st.h(0);
    let mut rng = StdRng::seed_from_u64(1);
    let (m0, _) = st.measure_z(0, &mut rng);
    let (m1, _) = st.measure_z(0, &mut rng);
    assert!(m0 == 0 || m0 == 1);
    assert!(m1 == 0 || m1 == 1);
}
