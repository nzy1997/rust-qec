"""Discover helpers, assertions and loops in the pinned standalone C++ suites."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib
import re


@dataclass(frozen=True)
class Token:
    text: str
    start: int
    end: int
    kind: str = "punctuation"


def digest(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def tokens(source: str) -> list[Token]:
    out = []
    i = 0
    while i < len(source):
        if source[i].isspace():
            i += 1
            continue
        if source.startswith("//", i):
            stop = source.find("\n", i)
            i = len(source) if stop < 0 else stop + 1
            continue
        if source.startswith("/*", i):
            stop = source.find("*/", i + 2)
            if stop < 0:
                raise ValueError("unterminated C++ block comment")
            i = stop + 2
            continue
        if source[i] == "#":
            while True:
                stop = source.find("\n", i)
                if stop < 0:
                    i = len(source)
                    break
                continued = source[stop - 1] == "\\"
                i = stop + 1
                if not continued:
                    break
            continue
        raw = re.match(r'(?:u8|u|U|L)?R"([^\s()\\]{0,16})\(', source[i:])
        if raw:
            delimiter = ")" + raw[1] + '"'
            stop = source.find(delimiter, i + raw.end())
            if stop < 0:
                raise ValueError("unterminated C++ raw literal")
            stop += len(delimiter)
            out.append(Token(source[i:stop], i, stop, "literal"))
            i = stop
            continue
        quoted = re.match(r"(?:u8|u|U|L)?([\"'])", source[i:])
        if quoted:
            stop = i + quoted.end()
            while stop < len(source) and source[stop] != quoted[1]:
                stop += 2 if source[stop] == "\\" else 1
            if stop >= len(source):
                raise ValueError("unterminated C++ quoted literal")
            stop += 1
            out.append(Token(source[i:stop], i, stop, "literal"))
            i = stop
            continue
        word = re.match(r"[A-Za-z_][A-Za-z_0-9]*|[0-9]+", source[i:])
        if word:
            stop = i + word.end()
            out.append(Token(word[0], i, stop, "word"))
            i = stop
            continue
        out.append(Token(source[i], i, i + 1))
        i += 1
    return out


def matching(items: list[Token]) -> dict[int, int]:
    stack = []
    pairs = {}
    closers = {"(": ")", "[": "]", "{": "}"}
    for index, token in enumerate(items):
        if token.kind == "literal":
            continue
        if token.text in closers:
            stack.append((index, token.text))
        elif token.text in closers.values():
            if not stack or closers[stack[-1][1]] != token.text:
                raise ValueError(f"unbalanced C++ delimiter at {token.start}")
            left, _ = stack.pop()
            pairs[left] = index
            pairs[index] = left
    if stack:
        raise ValueError("unclosed C++ delimiter")
    return pairs


def discover(source: str) -> dict:
    items = tokens(source)
    pairs = matching(items)
    functions = []
    calls = []
    cursor = 0
    boundary = 0
    while cursor < len(items):
        token = items[cursor]
        if token.text == ";":
            boundary = cursor + 1
        if token.text != "{" or token.kind == "literal":
            cursor += 1
            continue
        stop = pairs[cursor]
        if cursor < 2 or items[cursor - 1].text != ")":
            # These pinned suites have no namespaces/classes containing test
            # functions. Reject them rather than silently missing helpers.
            raise ValueError(f"unsupported top-level C++ scope at {token.start}")
        arguments = pairs[cursor - 1]
        name = items[arguments - 1].text
        if items[arguments - 1].kind != "word":
            raise ValueError("unsupported standalone function signature")
        start = items[boundary].start
        end = items[stop].end
        function = dict(name=name, line=source.count("\n", 0, start) + 1,
                        end_line=source.count("\n", 0, end) + 1,
                        source_sha256=digest(source[start:end]),
                        loops=[], assertions=[], helper_calls=[])
        ordinal = 0
        for j in range(cursor + 1, stop):
            at = items[j]
            if at.kind == "literal":
                continue
            if at.text in ("for", "while") and items[j + 1].text == "(":
                right = pairs[j + 1]
                function["loops"].append(dict(line=source.count("\n", 0, at.start) + 1,
                    header=source[at.start:items[right].end]))
            if at.text != "require" or items[j + 1].text != "(":
                continue
            right = pairs[j + 1]
            expression = source[at.start:items[right].end]
            ordinal += 1
            identity = f"{name}/require-{ordinal:03}"
            row = dict(id=identity, function=name,
                       line=source.count("\n", 0, at.start) + 1,
                       end_line=source.count("\n", 0, items[right].end) + 1,
                       expression_sha256=digest(expression), expression=expression)
            calls.append(row)
            function["assertions"].append(identity)
        functions.append(function)
        cursor = stop + 1
        boundary = cursor
    names = {f["name"] for f in functions}
    if len(names) != len(functions):
        raise ValueError("overloaded standalone helpers need an explicit identity")
    for function in functions:
        body = source.splitlines()[function["line"] - 1:function["end_line"]]
        local = tokens("\n".join(body))
        found = {local[j].text for j in range(len(local) - 1)
                 if local[j].kind == "word" and local[j + 1].text == "("
                 and local[j].text in names and local[j].text != function["name"]}
        function["helper_calls"] = sorted(found)
    raw_calls = sum(t.text == "require" and t.kind != "literal" and items[j + 1].text == "("
                    for j, t in enumerate(items[:-1]))
    if raw_calls != len(calls) + int("require" in names):
        raise ValueError("standalone assertion discovery omitted a require site")
    return dict(source_sha256=digest(source), functions=functions, assertions=calls,
                counts=dict(functions=len(functions), assertions=len(calls),
                            loop_headers=sum(len(f["loops"]) for f in functions)))
