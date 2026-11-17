"""Recompute observations using the verifiers at a pinned Git revision."""

from pathlib import Path
import json
import sys


def identity(remaining, raw):
    receipt = remaining.read_json(raw/"receipt.json")
    fields = ("rust_binary_sha256", "rust_source_sha256", "host")
    return dict({key: receipt[key] for key in fields}, cargo_lock_sha256=receipt["rust_source_inputs"]["Cargo.lock"],
                **{key: receipt[key] for key in ("cargo", "rustc", "python_environment") if key in receipt})


def main():
    source, name, manifest, directory = sys.argv[1:]
    sys.path.insert(0, source)
    import remaining_contracts as remaining
    from remaining_contracts import read_json, require

    plan = read_json(Path(manifest))
    raw = Path(directory)
    if name == "bh":
        import bh_benchmark
        from paper_probe_identity import source_inputs

        result = bh_benchmark.check_capture(plan, raw)
        require(read_json(raw/"receipt.json")["rust_source_inputs"] == source_inputs(Path(source).parent.parent),
                "BH capture source inputs differ from the pinned revision")
        result["build"] = identity(remaining, raw)
    elif name == "cultivation":
        import cultivation_benchmark
        import cultivation_peers
        import cultivation_reference

        require(plan["scope"] == "figure4-table2", "cultivation must use the full paper scope")
        summary = cultivation_benchmark.check_capture(plan, raw)
        peers = cultivation_peers.check(raw/"matched-peers")
        peer_receipt = read_json(raw/"matched-peers/receipt.json")
        require(peer_receipt["output_contract"] == "joint acceptance and target failure per attempted draw",
                "matched timing output contract changed")
        result = dict(summary=summary, observed=read_json(raw/"report.json"), peers=peers,
                      historical_curve=read_json(raw/"historical"/cultivation_reference.FIGURE),
                      failed_peer_attempts=peer_receipt["previous_failures"], build=identity(remaining, raw),
                      peer_host=peer_receipt["host"], peer_rust_binary_sha256=peer_receipt["rust_binary_sha256"])
    elif name == "decoder-workloads":
        import decoder_workloads_benchmark

        summary = decoder_workloads_benchmark.check_capture(plan, raw)
        coverage = read_json(Path(source)/"coverage.json")
        result = dict(summary=summary, coverage={row["id"]: row["status"] for row in coverage["cases"]}, observed={
            campaign: read_json(raw/campaign/"report.json")
            for campaign in decoder_workloads_benchmark.CAMPAIGNS
        }, builds={
            campaign: identity(remaining, raw/campaign) for campaign in decoder_workloads_benchmark.CAMPAIGNS
        })
    else:
        raise ValueError(f"unknown paper workload {name}")
    print(json.dumps(result, allow_nan=False))


if __name__ == "__main__":
    main()
