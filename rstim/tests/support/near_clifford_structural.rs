// Shared deterministic structural inputs, generated without external fixtures.
pub fn circuit(rank: usize, rounds: usize, signed: bool) -> String {
    assert!(rank >= 2 && rank % 2 == 0 && rank <= 16);
    let list = |start: usize, end: usize| {
        (start..end)
            .map(|q| q.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut text = format!("R {}\nH 0\nM 0\nR 0\n", list(0, 2 * rank));
    for round in 0..rounds {
        text += &format!("H {}\nT {}\n", list(0, rank), list(0, rank));
        for q in 0..rank {
            text += &format!("CX {q} {}\n", rank + (q + round) % rank);
        }
        text += &format!("DEPOLARIZE1(0.001) {}\n", list(0, 2 * rank));
        if signed {
            for q in (0..rank).step_by(2) {
                text += &format!("Z {q}\n");
            }
        }
        text += "MPP ";
        text += &(0..rank)
            .step_by(2)
            .map(|q| format!("X{q}*Z{}", q + 1))
            .collect::<Vec<_>>()
            .join(" ");
        text += "\nDETECTOR rec[-1]\nCX rec[-1] 0\n";
    }
    text += &format!(
        "MR {}\nM {}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        list(0, rank),
        list(0, 2 * rank)
    );
    text
}
