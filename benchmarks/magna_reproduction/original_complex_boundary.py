#!/usr/bin/env python3
"""Capture complete original complex-boundary methods and literal branch tables."""
# SPDX-License-Identifier: MPL-2.0
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest
import reference
from witness_environment import native_reference
from witness_paths import admit_destinations


def capture(upstream,helper):
    upstream,helper=Path(upstream).resolve(),Path(helper).resolve()
    helper_hash=reference.digest(helper)
    if "_native_testing" in sys.modules:raise ValueError("fresh Python process required")
    with native_reference(upstream) as captured:
        spec=importlib.util.spec_from_file_location("_native_testing",helper)
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        sys.modules["_native_testing"]=module
        sys.path[:0]=[str(upstream),str(upstream/"tests/core")]
        from magna.operations import BoundarySampler,InstrumentIR,StabilizerKet,ControlNode,PhaseFeedback
        import test_boundary_sampling as boundary
        original_sample=BoundarySampler.sample
        original_exhaustive=boundary.BoundarySamplingTests.exhaustive
        observations,failures,tables=[],[],[]
        def scalar(value):return None if value is None else value.to_dict()
        def sample(self,rng,*args,**kwargs):
            before=hashlib.sha256(repr(rng.getstate()).encode()).hexdigest()
            try:draw=original_sample(self,rng,*args,**kwargs)
            except Exception as error:
                failures.append(dict(type=type(error).__name__,message=str(error)));raise
            observations.append(dict(route=self.route,required=self.required,reports=draw.reports,true=draw.true,
                mass=scalar(draw.mass),acceptance=scalar(draw.acceptance),error=scalar(draw.error),
                moments=[scalar(v) for v in draw.moments],controls_executed=draw.diagnostics.get("controls_executed"),
                before_rng_sha256=before,after_rng_sha256=hashlib.sha256(repr(rng.getstate()).encode()).hexdigest()))
            return draw
        def exhaustive(self,ir,faults,flips):
            result=original_exhaustive(self,ir,faults,flips)
            tables.append(dict(method=self._testMethodName,faults=faults,flips=flips,
                records=ir.record_names,rows=[dict(reports=r,mass=scalar(p),moments=[scalar(v) for v in m]) for r,p,m in result]))
            return result
        ids=["test_boundary_sampling.BoundarySamplingTests."+name for name in (
            "test_affine_phase_arithmetic_is_direct_and_odd_control_is_complex",
            "test_complex_feedback_global_queries_with_future_postselection",
            "test_noncommuting_reference_uses_exact_global_queries",
            "test_undeclared_callback_and_invalid_family_fail")]
        ids+=["test_native_boundary_service.NativeBoundaryServiceTests.test_causal_controller_order_rng_and_terminal_masses"]
        BoundarySampler.sample=sample;boundary.BoundarySamplingTests.exhaustive=exhaustive
        try:result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames(ids))
        finally:BoundarySampler.sample=original_sample;boundary.BoundarySamplingTests.exhaustive=original_exhaustive
        if not result.wasSuccessful() or result.testsRun!=5 or result.skipped:
            raise ValueError("complete original methods did not pass")
        if len(tables)!=6 or len(observations)!=50 or len(failures)!=1:
            raise ValueError("complete original query schedule changed")
        # The fifth method has a reference sampler, not an exhaustive helper.
        # Retain an additional literal full table of its exact source fixture.
        ir=InstrumentIR((0,1),(1,),(
            dict(id="t",kind="T",qubits=[0]),dict(id="a",kind="MX",qubits=[0],record="a"),
            dict(id="c",kind="CONTROL"),dict(id="b",kind="MX",qubits=[1],record="b"),
            dict(id="drop",kind="DISCARD",qubits=[0])),StabilizerKet.positive(2,(1,2)),
            {"c":ControlNode(("a",),"test",lambda r:(dict(kind="S",qubits=[1],exponent=2*r["a"]),),PhaseFeedback((1,)))})
        faults=(dict(after="c",qubit=1,pauli="Y"),);flips=("a",)
        rows=original_exhaustive(boundary.BoundarySamplingTests(),ir,faults,flips)
        tables.append(dict(method="causal_controller_order_supplemental_literal_table",faults=faults,flips=flips,
            records=ir.record_names,rows=[dict(reports=r,mass=scalar(p),moments=[scalar(v) for v in m]) for r,p,m in rows]))
        document=dict(schema="rmagna.original-complex-boundary.v1",upstream_commit=captured.manifest["upstream"]["commit"],
            reference_sha256=captured.manifest_sha256,native_frontend_sha256=captured.native_sha256,
            native_helper_sha256=helper_hash,producer_sha256=reference.digest(Path(__file__)),
            source_sha256={p:captured.manifest["upstream"]["files"][p] for p in
                ("tests/core/test_boundary_sampling.py","tests/core/test_native_boundary_service.py","tests/support/boundary_sampler.py")},
            methods=ids,original_methods_passed=5,tables=tables,observations=observations,failures=failures)
    if reference.digest(helper)!=helper_hash:raise ValueError("identified helper changed")
    return document


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream",type=Path,required=True)
    parser.add_argument("--native-helper",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True)
    args=parser.parse_args()
    try:
        destination,=admit_destinations([args.out],args.upstream,protected_roots=[reference.HERE,args.native_helper.parent])
        result=capture(args.upstream,args.native_helper)
        with destination.open("x",encoding="utf-8") as stream:
            json.dump(result,stream,separators=(",",":"),allow_nan=False);stream.write("\n")
    except (ValueError,OSError) as error:parser.error(str(error))
    print("all5 original methods passed;50 draws,7 complete literal tables,1 expected mismatch")


if __name__=="__main__":main()
