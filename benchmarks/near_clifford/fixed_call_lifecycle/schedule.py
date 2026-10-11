"""Complete fixed-call matrix; this schedule never adapts to measurements."""
from itertools import permutations, product
INPUTS = {
    "msc_d3_inject_cultivate_p1e-3": "90a7d841e003e5ee38137cd9a3eb6529bb552e49c424bc6b0932a27d97cdb41f",
    "msc_d5_inject_cultivate_p1e-3": "c2b4566917bd9bf27a5705284dac02700ef0dcc7c03c91066670db376d633a6d",
}
ROLES = ("baseline", "candidate", "control")
CELLS = tuple(product(INPUTS, (1, 64, 1024), ("strict", "fused"), ("default", "off")))
HORIZONS = (1, 2, 4, 8, 16, 32, 64, 128, 256)
ORDERS = tuple(permutations(ROLES))
def schedule(action):
    if action not in ("validate", "bench"):
        raise ValueError("unknown action")
    rounds = 1 if action == "validate" else 12
    events = []
    for round_index in range(rounds):
        cells = CELLS[round_index:] + CELLS[:round_index]
        for cell in cells:
            cell_index = CELLS.index(cell)
            order = ORDERS[(round_index + cell_index) % 6]
            for role in order:
                events.append({"id": f"{action}-r{round_index:02}-c{cell_index:02}-{role}",
                    "action": action, "round": round_index, "cell": list(cell),
                    "role": role, "role_order": list(order)})
    return events
