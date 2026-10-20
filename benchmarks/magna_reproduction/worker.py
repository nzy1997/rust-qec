"""One isolated upstream task; only this child imports the reference simulator."""

import argparse
import hashlib
import importlib.metadata
import json
import platform
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--shots", type=int, required=True)
    parser.add_argument("--seed", type=int, required=True)
    args = parser.parse_args()
    source = args.upstream.resolve()
    task = (source / "examples" / args.case).resolve()
    if not task.is_relative_to(source / "examples"):
        parser.error("case escapes the reference examples directory")
    config = json.loads((task / "run.json").read_text(encoding="utf-8"))
    sys.path[:0] = [str(source / "core"), str(task)]
    from magna import _native, load_controllers, run

    binary = Path(_native.__file__).resolve()
    if not binary.is_relative_to(source / "core/magna"):
        raise ValueError("reference native extension was imported outside the pinned source tree")
    native_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
    controllers = load_controllers(task / config["controllers"]) if config.get("controllers") else None
    result = run(task / config["circuit"], task=task / config["task"], controllers=controllers,
                 options=config.get("options", {}), sampling=config.get("sampling", {"method": "physical"}),
                 shots=args.shots, seed=args.seed)
    if hashlib.sha256(binary.read_bytes()).hexdigest() != native_sha256:
        raise ValueError("reference native extension changed during the task")
    dependencies = {}
    for name in ("numpy", "stim", "pymatching", "scipy"):
        try:
            dependencies[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            dependencies[name] = None
    print(json.dumps({"result": result, "native_sha256": native_sha256,
                      "python": sys.version, "platform": platform.platform(),
                      "dependencies": dependencies}, allow_nan=False))


if __name__ == "__main__":
    main()
