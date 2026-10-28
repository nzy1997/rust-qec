"""Conservative Rust witness declarations and auditable libtest observations.

This validates supplied evidence, not the honesty of its producer. Keep the
original compiler/process receipts alongside every evidence file for review.
"""

from __future__ import annotations

import hashlib
import re
from pathlib import Path


class CoverageError(ValueError):
    """A coverage input cannot support the claimed inventory or execution."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CoverageError(message)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fields(value: object, keys: set[str], context: str) -> dict:
    require(isinstance(value, dict) and set(value) == keys, f"invalid fields: {context}")
    return value


def text(value: object, context: str) -> str:
    require(isinstance(value, str) and bool(value.strip()), f"missing text: {context}")
    return value


def safe_file(root: Path, relative: str) -> Path:
    require(isinstance(relative, str) and bool(relative), "invalid source path")
    require(not Path(relative).is_absolute() and "\\" not in relative
            and all(p not in {"", ".", ".."} for p in relative.split("/")),
            f"invalid source path: {relative}")
    path = root / relative
    require(not any(p.is_symlink() for p in [path, *path.parents]),
            f"symlink in source path: {relative}")
    require(path.is_file(), f"missing source file: {relative}")
    return path


def rust_tokens(source: str) -> list[str]:
    """Discard comments/literals before looking for unconditional flat tests."""
    tokens = []
    i = 0
    while i < len(source):
        if source[i].isspace():
            i += 1
            continue
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end + 1
            continue
        if source.startswith("/*", i):
            depth = 1
            i += 2
            while depth and i < len(source):
                if source.startswith("/*", i):
                    depth += 1
                    i += 2
                elif source.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            require(depth == 0, "unterminated Rust comment")
            continue
        raw = re.match(r'(?:br|cr|r)(#*)"', source[i:])
        if raw:
            end = source.find('"' + raw[1], i + raw.end())
            require(end >= 0, "unterminated Rust raw string")
            i = end + 1 + len(raw[1])
            tokens.append("LITERAL")
            continue
        quoted = re.match(r'(?:b|c)?"', source[i:])
        if quoted:
            i += quoted.end()
            while i < len(source) and source[i] != '"':
                i += 2 if source[i] == "\\" else 1
            require(i < len(source), "unterminated Rust string")
            i += 1
            tokens.append("LITERAL")
            continue
        char = re.match(r"(?:b)?'(?:\\u\{[0-9a-fA-F_]+\}|\\x[0-9a-fA-F]{2}|\\.|[^'\\\n])'", source[i:])
        if char:
            i += char.end()
            tokens.append("LITERAL")
            continue
        word = re.match(r"(?:r#)?[A-Za-z_][A-Za-z_0-9]*", source[i:])
        if word:
            tokens.append(word[0])
            i += word.end()
        else:
            tokens.append(source[i])
            i += 1
    return tokens


def flat_tests(source: str) -> set[str]:
    """Only explicit, unconditional, top-level #[test] functions are witnesses.

    Raw/non-ASCII function identifiers, inline modules, macros, cfg/ignore
    attributes and generated tests require a future explicit extension; an
    identifier prefix or text match inside one is never accepted.
    """
    tokens = rust_tokens(source)
    delimiters = []
    attributes = []
    names = set()
    functions = set()
    i = 0
    while i < len(tokens):
        token = tokens[i]
        require(delimiters or tokens[i:i + 3] != ["#", "!", "["],
                "inner Rust attributes require explicit witness support")
        if not delimiters and token == "#" and tokens[i + 1:i + 2] == ["["]:
            end = i + 2
            brackets = 1
            while end < len(tokens) and brackets:
                brackets += (tokens[end] == "[") - (tokens[end] == "]")
                end += 1
            require(brackets == 0, "unterminated Rust attribute")
            attributes.append(tokens[i + 2:end - 1])
            i = end
            continue
        if not delimiters and token == "fn":
            # A complete ordinary ASCII identifier must precede the parameter
            # list. Raw/Unicode prefixes and function-pointer types are not
            # supported function declarations and must never invent a name.
            if (i + 2 < len(tokens)
                    and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[i + 1])
                    and tokens[i + 2] == "("):
                name = tokens[i + 1]
                require(name not in functions, f"ambiguous Rust function: {name}")
                functions.add(name)
                if attributes == [["test"]]:
                    names.add(name)
            attributes = []
        elif not delimiters and token not in {"pub", "async", "unsafe", "extern"}:
            attributes = []
        if token in {"{", "(", "["}:
            delimiters.append(token)
        elif token in {"}", ")", "]"}:
            require(bool(delimiters) and delimiters.pop() == {"}": "{", ")": "(", "]": "["}[token],
                    "unbalanced Rust delimiters")
        i += 1
    require(not delimiters, "unbalanced Rust delimiters")
    return names


def witness(root: Path, value: object) -> tuple[str, str]:
    row = fields(value, {"package", "target", "test", "source_sha256"}, "Rust witness")
    require(row["package"] == "rmagna", "unsupported Rust package")
    for key in ("target", "test"):
        require(isinstance(row[key], str) and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", row[key]) is not None,
                f"invalid or nested Rust {key}: {row[key]!r}")
    path = safe_file(root, f"rmagna/tests/{row['target']}.rs")
    require(sha(path) == row["source_sha256"], f"Rust source SHA-256 mismatch: {row['target']}")
    require(row["test"] in flat_tests(path.read_text(encoding="utf-8")),
            f"nonexistent or unsupported Rust test: {row['target']}::{row['test']}")
    return row["target"], row["test"]


def source_identity(root: Path) -> dict[str, str]:
    paths = ["Cargo.toml", "Cargo.lock"]
    paths.extend(p.relative_to(root).as_posix() for p in (root / "rmagna").rglob("*")
                 if p.is_file() or p.is_symlink())
    if (root / ".cargo").exists():
        paths.extend(p.relative_to(root).as_posix() for p in (root / ".cargo").rglob("*")
                     if p.is_file() or p.is_symlink())
    return {name: sha(safe_file(root, name)) for name in sorted(paths)}


def process(value: object, context: str) -> dict:
    row = fields(value, {"stdout", "stderr", "exit_code"}, context)
    require(all(isinstance(row[k], str) for k in ("stdout", "stderr"))
            and type(row["exit_code"]) is int, f"invalid process observations: {context}")
    return row


def execution(root: Path, supplied: object, required: set[tuple[str, str]]) -> dict:
    """Retain raw observations even when they cannot establish successful runs."""
    result = {"provided": supplied is not None, "source_matches": False,
              "passed": [], "failed": [], "missing": [], "errors": [], "raw": supplied}
    outcomes = {}
    if supplied is not None:
        try:
            data = fields(supplied, {"schema", "source_files", "targets"}, "Rust run evidence")
            require(data["schema"] == "rstim.magna-rust-tests.v1", "unsupported Rust run schema")
            expected = source_identity(root)
            result["expected_source_files"] = expected
            result["source_matches"] = data["source_files"] == expected
            require(result["source_matches"], "Rust run source identity mismatch")
            require(isinstance(data["targets"], list), "Rust targets must be an array")
            seen = set()
            for row in data["targets"]:
                fields(row, {"package", "target", "binary", "list", "run"}, "Rust target run")
                require(row["package"] == "rmagna", "unsupported run package")
                target = row["target"]
                require(isinstance(target, str) and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", target) is not None,
                        "invalid run target")
                require(target not in seen, f"duplicate Rust run target: {target}")
                seen.add(target)
                binary = fields(row["binary"], {"path", "sha256"}, "Rust binary")
                path = Path(text(binary["path"], "Rust binary path"))
                require(path.is_absolute() and path.is_file() and sha(path) == binary["sha256"],
                        f"Rust binary SHA-256 mismatch: {target}")
                listing = process(row["list"], "Rust --list")
                run = process(row["run"], "Rust run")
                require(listing["exit_code"] == 0, f"Rust test listing failed: {target}")
                names = re.findall(r"^([A-Za-z_][A-Za-z_0-9:]*)\: test$", listing["stdout"], re.M)
                require(len(names) == len(set(names)) and bool(names), f"invalid Rust test listing: {target}")
                list_summary = re.findall(r"^(\d+) tests?, 0 benchmarks$", listing["stdout"], re.M)
                require(list_summary == [str(len(names))], f"Rust test listing count mismatch: {target}")
                events = re.findall(r"^test ([A-Za-z_][A-Za-z_0-9:]*) \.\.\. (ok|FAILED|ignored)(?:,.*)?$",
                                    run["stdout"], re.M)
                require(len(events) == len({name for name, _ in events})
                        and all(name in names for name, _ in events), f"invalid Rust test results: {target}")
                summaries = re.findall(
                    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;.*$",
                    run["stdout"], re.M)
                require(len(summaries) == 1, f"missing Rust test result summary: {target}")
                status, passed, failed, ignored, measured, filtered = summaries[0]
                counts = {kind: sum(outcome == kind for _, outcome in events) for kind in ("ok", "FAILED", "ignored")}
                require([int(passed), int(failed), int(ignored)] == [counts[k] for k in ("ok", "FAILED", "ignored")]
                        and int(measured) == 0 and len(events) + int(filtered) == len(names),
                        f"Rust run counts mismatch: {target}")
                require((status == "ok" and int(failed) == 0 and run["exit_code"] == 0)
                        or (status == "FAILED" and int(failed) > 0 and run["exit_code"] != 0),
                        f"Rust run status mismatch: {target}")
                for name, outcome in events:
                    outcomes[target, name] = outcome
                if run["exit_code"] != 0:
                    result["errors"].append(f"Rust target run failed: {target}")
        except (CoverageError, OSError, UnicodeError) as error:
            result["errors"].append(str(error))
    for target, name in sorted(required):
        label = f"rmagna::{target}::{name}"
        outcome = outcomes.get((target, name))
        if outcome == "ok":
            result["passed"].append(label)
        elif outcome == "FAILED":
            result["failed"].append(label)
        else:
            result["missing"].append(label)
    return result
