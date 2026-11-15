"""Finite exact checks for the paper-specific proposal distributions."""
from fractions import Fraction
import math
from pathlib import Path

import reference
from paper_proposals import (CultivationProposal, ResidualClassProposal,
                             check_cultivation_model, check_residual_model, require)

HERE = Path(__file__).resolve().parent


def check_finite(manifest):
    import json
    pinned = reference.load_manifest()
    require(manifest["schema"] == "rmagna.paper-proposals.v1", "unsupported proposal manifest")
    require(manifest["reference_sha256"] == reference.digest(reference.MANIFEST), "reference changed")
    require(manifest["paper"] == pinned["paper"]["arxiv"], "paper version changed")
    require(manifest["historical_driver_source"] == "hash_only_distribution_reconstruction",
            "historical source/RNG identity must not be claimed")
    binding = manifest["residual_fixture"]
    path = reference.relative_file(HERE, binding["file"])
    require(path.stat().st_size <= 1 << 20 and reference.digest(path) == binding["sha256"],
            "residual fixture changed")
    data = json.loads(path.read_text())
    require(data["schema"] == "rmagna.original-logical-residuals.v1" and
            data["paper"] == pinned["paper"]["arxiv"] and
            data["paper_archive_sha256"] == pinned["paper"]["archive_sha256"] and
            data["upstream_commit"] == pinned["upstream"]["commit"] and
            data["reference_sha256"] == manifest["reference_sha256"], "fixture provenance changed")
    require(data["producer_sha256"] == reference.digest(HERE/"original_logical_residuals.py"),
            "residual producer changed")
    source_files = pinned["upstream"]["files"] | pinned["upstream"].get("decoder_files", {})
    require(set(data["source_sha256"]) == {"examples/logical-distillation/decoder/patch.py",
                                          "examples/logical-distillation/decoder/decoder.py"},
            "residual source scope changed")
    for name, digest in data["source_sha256"].items():
        require(source_files[name] == digest, "residual source changed")
    require(len(data["paper_source_sha256"]) == 6, "deposited source scope changed")
    for name, digest in data["paper_source_sha256"].items():
        require(pinned["paper"]["files"][name] == digest, "deposited proposal data changed")

    cultivation = manifest["cultivation"]
    require(cultivation["extra_faults"] == "3" and cultivation["mixture"] == "19/20" and
            cultivation["closing_sites"] == {"color3":1,"color5":1,"rp2-d3-T":1,"rp2-d5-T":2},
            "paper cultivation settings changed")
    for name, record in data["cultivation"].items():
        settings = record["proposal"]
        expected = Fraction(str(record["rate"])) + Fraction(3, record["noise_categories"])
        require(abs(settings["q"]-float(expected)) <= math.ulp(float(expected)),
                "historical p+3/N setting changed")
        require(settings["extra_faults"] == 3 and settings["closure_mixture"] == .95 and
                settings["closing_channels"] == cultivation["closing_sites"][name],
                "historical closure setting changed")
    cases = manifest["finite_cultivation_cases"]
    require(isinstance(cases,list) and 1 <= len(cases) <= 32 and
            len({c["id"] for c in cases}) == len(cases), "invalid finite cultivation cases")
    counts = []
    for case in cases:
        model = CultivationProposal(case["signatures"],case["rate"],
            channel_weights=case.get("channel_weights"), closing_sites=case["closing_sites"],
            mixture=cultivation["mixture"],extra_faults=cultivation["extra_faults"],
            offset=case.get("offset",0))
        counts.append(dict(id=case["id"], **check_cultivation_model(model)))
    require({c["closing_sites"] for c in cases} == {1,2}, "both closing policies must execute")

    logical = manifest["logical"]
    require(logical == dict(proposal_rate="1/100",nonidentity="1/5",
                rates=["1/1000","3/1000","1/100"],patches_per_draw=15),
            "paper logical proposal settings changed")
    model = ResidualClassProposal(data["x_residual"],data["z_residual"],
                proposal_rate=logical["proposal_rate"],nonidentity=logical["nonidentity"])
    geometry = data["geometry"]
    logical_x = sum(1 << (2*q) for q in geometry["lx"])
    logical_z = sum(1 << (2*q+1) for q in geometry["lz"])
    for word, label in [(0,0),(logical_x,1),(logical_z,2),(logical_x^logical_z,3)]:
        require(model.classify(word)[0] == label, "logical IXZY axes changed")
    # The equal historical X/Z weight counts alone cannot distinguish a
    # swapped logical-axis map. Check stabilizer cosets and logical axes too.
    for table, supports in [(model.x,geometry["xchecks"]),(model.z,geometry["zchecks"])]:
        for support in supports:
            mask = sum(1 << q for q in support)
            require(all(table[word] == table[word^mask] for word in range(512)),
                    "residual classifier changes within a stabilizer coset")
    result = check_residual_model(model,data["counts_IXZY"])
    require(list(map(float,model.class_proposal)) == data["metadata"]["proposal"]["probabilities"],
            "historical residual-class proportions changed")
    require({r["p"] for r in data["exact_rows"]} == {"0.001","0.003","0.01"}, "exact rate grid changed")
    for row in data["exact_rows"]:
        _, px, pz, py = model.classes_at(row["p"])
        q = pz+(px+py)/2
        require(q == Fraction(row["effective_q_exact"]), "exact physical class oracle changed")
        s = 1-2*q
        acceptance = (1+15*s**8)/16
        accepted_error = (1+15*s**8-15*s**7-s**15)/32
        for key, value in [("acceptance",acceptance),("accepted_error",accepted_error),
                           ("conditional_error",accepted_error/acceptance)]:
            require(value == Fraction(row["exact"][key]), f"deposited exact {key} oracle changed")
    leading = (data["counts_IXZY"][2][2] +
               Fraction(data["counts_IXZY"][1][2]+data["counts_IXZY"][3][2],2))/9
    require(leading == Fraction(data["metadata"]["effective_q_leading"]) and
            35*leading**3 == Fraction(data["metadata"]["output_error_leading"]),
            "leading logical error coefficients changed")
    return dict(status="PASS", scope="finite_exact_proposals_not_paper_campaign",
                cultivation=counts, logical=result, historical_source_identity=False,
                decoder_runtime_integration="not_yet_validated")


def check_with_runtime(manifest, binary):
    import tempfile
    from paper_benchmark import run_process
    require(binary is not None, "set MAGNA_PAPER_PROPOSAL_PROBE or --rust-probe to an existing paper_proposal_probe executable")
    result = check_finite(manifest)
    binary = Path(binary).resolve(strict=True)
    before = reference.digest(binary)
    with tempfile.TemporaryDirectory(prefix="magna-proposal-check-") as directory:
        stage = run_process([str(binary)], 60, directory)
    require(stage["status"] == "complete", f"Rust proposal integration failed: {stage}")
    payload = stage["payload"]
    expected = dict(schema="rmagna.paper-proposal-integration.v1",status="PASS",samples=768,
        true_reported_differ=384,decoder_calls=768,rates=3,causal_strategies=2,
        closing_policies=2,patterns_per_policy=64,
        fixture_sha256=reference.digest(HERE.parents[1]/"rmagna/tests/data/paper-proposal-runtime.json"),
        source_sha256=reference.digest(HERE.parents[1]/"rmagna/tests/support/paper_proposal_probe.rs"))
    require(payload == expected, "Rust proposal integration result/source changed")
    require(reference.digest(binary) == before, "Rust proposal probe changed during execution")
    result.update(decoder_runtime_integration="PASS",rust_integration=stage,rust_binary_sha256=before)
    return result
