from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import tomllib


@dataclass(frozen=True)
class RunnerSpec:
    name: str
    language: str
    impl_key: str
    params: dict[str, object]


def load_spec(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def python_runners_from_spec(spec: dict[str, object]) -> list[RunnerSpec]:
    runners = []
    for runner in spec.get("runner", []):
        if runner["language"] == "python":
            runners.append(
                RunnerSpec(
                    name=runner["name"],
                    language=runner["language"],
                    impl_key=runner["impl_key"],
                    params=runner["params"],
                )
            )
    return runners
