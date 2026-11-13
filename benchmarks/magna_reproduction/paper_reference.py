"""One isolated prescribed no-fault reference draw; no Rust code is imported."""

import argparse
import contextlib
import importlib
import json
from pathlib import Path
import random
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from witness_environment import native_reference


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--seed", type=int, required=True)
    args = parser.parse_args()
    with native_reference(args.upstream) as captured:
        if args.case not in captured.manifest["tasks"]:
            parser.error("case is outside the pinned task inventory")
        root = args.upstream.resolve() / "examples" / args.case
        sys.path.insert(0, str(root))
        from magna import ControllerRegistry, compile_circuit
        from magna.file_api import NoiseDraw
        config = json.loads((root / "run.json").read_text())
        registry = ControllerRegistry()
        calls = []
        if config.get("controllers"):
            bindings = json.loads((root / config["controllers"]).read_text())["bindings"]
            for name, spec in bindings.items():
                module, function = spec["import"].split(":")
                with contextlib.redirect_stdout(sys.stderr):
                    callback = getattr(importlib.import_module(module), function)

                def recorded(bits, config, callback=callback, name=name):
                    with contextlib.redirect_stdout(sys.stderr):
                        result = callback(bits, config)
                    calls.append(dict(name=name, inputs=list(map(bool, bits)), outputs=list(map(bool, result))))
                    return result

                registry.register(name, recorded, version=spec["version"], config=spec.get("config", {}))
        compiled = compile_circuit(root / config["circuit"], task=root / config["task"],
            controllers=registry, options=dict(config["options"], history_sampler="exact_marginal"))
        with contextlib.redirect_stdout(sys.stderr):
            draw = compiled.sample(random.Random(args.seed), noise=NoiseDraw({}, ()))
        if draw["sampled_records"]:
            raise ValueError("additional requested records need an explicit replay contract")
        exact = draw["exact"]
        def scalar(value):
            return None if value is None else dict(coefficients=list(map(str, value["coefficients"])), power=value["power"])
        result = {"raw_" + key: scalar(exact[key]) for key in ("acceptance", "error", "target_success")}
        result["history_mass"] = scalar(exact["history_mass"])
        result["target_normalization"] = scalar(exact.get("target_normalization", dict(coefficients=[1, 0, 0, 0], power=0)))
        result["controller_calls"] = calls
        result["native_sha256"] = captured.native_sha256
        result["python"] = sys.version
        print(json.dumps(result, allow_nan=False))


if __name__ == "__main__":
    main()
