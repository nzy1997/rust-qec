use rstim::parser::parse_lines;
use rstim::executor::Executor;

fn exec(circuit: &str) -> Vec<bool> {
    let instrs = parse_lines(circuit).unwrap();
    let mut executor = Executor::from_instrs(instrs).unwrap();
    let mut rng = rand::thread_rng();
    executor.run(&mut rng).unwrap().measurements
}

#[test]
fn c_xyz_period_3() {
    let m = exec("R 0\nC_XYZ 0\nC_XYZ 0\nC_XYZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn c_xyz_x_to_y() {
    let m = exec("R 0\nX 0\nC_XYZ 0\nC_XYZ 0\nC_XYZ 0\nX 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn c_zyx_inverse_of_c_xyz() {
    let m = exec("R 0\nC_XYZ 0\nC_ZYX 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn h_nxy_period_2() {
    let m = exec("R 0\nH_NXY 0\nH_NXY 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn h_nxz_period_2() {
    let m = exec("R 0\nH_NXZ 0\nH_NXZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn h_nyz_period_2() {
    let m = exec("R 0\nH_NYZ 0\nH_NYZ 0\nM 0");
    assert_eq!(m, vec![false]);
}

#[test]
fn all_c_gates_period_3() {
    for gate in &["C_XYZ", "C_ZYX", "C_NXYZ", "C_NZYX", "C_XNYZ", "C_XYNZ", "C_ZNYX", "C_ZYNX"] {
        let circuit = format!("R 0\n{g} 0\n{g} 0\n{g} 0\nM 0", g = gate);
        let m = exec(&circuit);
        assert_eq!(m, vec![false], "gate {gate} is not period-3");
    }
}

#[test]
fn all_h_n_gates_period_2() {
    for gate in &["H_NXY", "H_NXZ", "H_NYZ"] {
        let circuit = format!("R 0\n{g} 0\n{g} 0\nM 0", g = gate);
        let m = exec(&circuit);
        assert_eq!(m, vec![false], "gate {gate} is not period-2");
    }
}

#[test]
fn c_xyz_c_zyx_identity() {
    for g1 in &["C_XYZ", "C_NXYZ", "C_XNYZ", "C_XYNZ"] {
        for g2 in &["C_ZYX", "C_NZYX", "C_ZNYX", "C_ZYNX"] {
            let circuit = format!("R 0\n{g1} 0\n{g1} 0\n{g1} 0\nM 0");
            let m = exec(&circuit);
            assert_eq!(m, vec![false], "{g1} cubed should be identity on |0>");

            let circuit2 = format!("R 0\n{g2} 0\n{g2} 0\n{g2} 0\nM 0");
            let m2 = exec(&circuit2);
            assert_eq!(m2, vec![false], "{g2} cubed should be identity on |0>");
        }
    }
}
