"""Regenerate the pinned distance-3, 3-round SI1000 fixtures with Stim."""

from pathlib import Path

import stim


STIM_VERSION = "1.15.0"
P = 0.002


def si1000_reference(basis):
    source = stim.Circuit.generated(
        f"surface_code:rotated_memory_{basis}", distance=3, rounds=3
    ).flattened()
    coords = source.get_final_qubit_coordinates()
    qubit_ids = {q: i for i, q in enumerate(sorted(coords, key=coords.get))}
    all_qubits = set(qubit_ids.values())
    circuit = stim.Circuit()
    for q, i in qubit_ids.items():
        circuit.append("QUBIT_COORDS", [i], coords[q])

    circuit.append("R", sorted(all_qubits))
    circuit.append("X_ERROR", sorted(all_qubits), 2 * P)

    def moment(gate, targets):
        circuit.append("TICK")
        if gate == "M":
            circuit.append("X_ERROR", targets, 5 * P)
        circuit.append(gate, targets)
        if gate == "CZ":
            circuit.append("DEPOLARIZE2", targets, P)
        elif gate == "R":
            circuit.append("X_ERROR", targets, 2 * P)
        spectators = sorted(all_qubits.difference(targets))
        # An H moment depolarizes every qubit at the same rate.
        idle = sorted(all_qubits) if gate == "H" else spectators
        if idle:
            circuit.append("DEPOLARIZE1", idle, P / 10)
        if gate in {"R", "M"} and spectators:
            circuit.append("DEPOLARIZE1", spectators, 2 * P)

    pending_h = set()
    record_order = []
    detectors = []
    for inst in source:
        name = inst.name
        targets = inst.targets_copy()
        args = inst.gate_args_copy()
        if name != "DETECTOR":
            for det_coords, det_targets in sorted(detectors):
                circuit.append("DETECTOR", det_targets, det_coords)
            detectors.clear()

        if name in {"QUBIT_COORDS", "R", "TICK"}:
            continue
        if name in {"DETECTOR", "OBSERVABLE_INCLUDE"}:
            indices = sorted(record_order[target.value] - len(record_order) for target in targets)
            records = [stim.target_rec(index) for index in indices]
            if name == "DETECTOR":
                detectors.append((args, records))
            else:
                circuit.append(name, records, args)
            continue

        qubits = [qubit_ids[target.value] for target in targets]
        if name == "RX":
            moment("H", sorted(qubits))
        elif name == "H":
            pending_h.symmetric_difference_update(qubits)
        elif name == "CX":
            # Expand CX into H-CZ-H. Delay each H past unrelated gates
            # so pending H pairs on a wire can cancel.
            cx_targets = qubits[1::2]
            pending_h.symmetric_difference_update(cx_targets)
            before_cz = pending_h.intersection(qubits)
            if before_cz:
                moment("H", sorted(before_cz))
            pending_h.difference_update(before_cz)
            moment("CZ", qubits)
            pending_h.symmetric_difference_update(cx_targets)
        elif name in {"MR", "M", "MX"}:
            if name == "MX":
                pending_h.symmetric_difference_update(qubits)
            if pending_h:
                moment("H", sorted(pending_h))
                pending_h.clear()
            sorted_qubits = sorted(qubits)
            new_indices = {q: i for i, q in enumerate(sorted_qubits, start=len(record_order))}
            record_order.extend(new_indices[q] for q in qubits)
            moment("M", sorted_qubits)
            if name == "MR":
                moment("R", sorted_qubits)
        else:
            raise ValueError(f"unexpected instruction: {inst}")
    return circuit


if __name__ == "__main__":
    if stim.__version__ != STIM_VERSION:
        raise SystemExit(f"expected stim=={STIM_VERSION}, found {stim.__version__}")
    output = Path(__file__).resolve().parent
    for basis in "xz":
        circuit = si1000_reference(basis)
        stem = f"rotated_memory_{basis}_d3_r3"
        (output / f"{stem}.stim").write_text(f"{circuit}\n", encoding="ascii")
        dem = circuit.detector_error_model(decompose_errors=False).flattened()
        (output / f"{stem}.dem").write_text(f"{dem}\n", encoding="ascii")
