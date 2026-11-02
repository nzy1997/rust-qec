"""Read-only destination admission before a numerical witness capture starts."""

from pathlib import Path


RUST_INPUT_ROOT = Path(__file__).resolve().parents[2] / "rmagna"


def admit_destinations(outputs, upstream, *, protected_roots=()):
    """Return fresh, distinct canonical paths outside all protected inputs."""
    lexical = [Path(path).absolute() for path in outputs]
    if not lexical or any(path.exists() or path.is_symlink() for path in lexical):
        raise ValueError("witness destinations must be fresh, including existing links")
    resolved = [path.resolve() for path in lexical]
    if len(set(resolved)) != len(resolved):
        raise ValueError("witness destinations must be distinct after resolving aliases")
    roots = [Path(upstream).resolve(), RUST_INPUT_ROOT.resolve(),
             *(Path(root).resolve() for root in protected_roots)]
    if any(path.is_relative_to(root) for path in resolved for root in roots):
        raise ValueError("witness destinations must be outside protected input trees")
    if any(not path.parent.is_dir() for path in resolved):
        raise ValueError("witness destination parents must be existing directories")
    return resolved
