"""Explicit bounded native options and strict diagnostic receipt validation."""
import sys

FAMILIES = ("probability", "density", "moment")
MODES = ("cold", "trace", "fixed", "catalogue", "native", "native_injection")
# CLI suffix, effective JSON path, default, wide integer encoding.
LIMITS = (
    ("native-max-quotient-rank", "selection.max_quotient_rank", 8, False),
    ("native-max-core-table-bytes", "selection.max_core_table_bytes", 64 << 20, False),
    ("native-max-signature-work", "selection.max_signature_work", 64 << 20, True),
    ("native-max-transform-work", "selection.max_transform_work", 64 << 20, True),
    ("native-max-construction-terms", "selection.construction.max_terms", 1 << 20, False),
    ("native-max-construction-entries", "selection.construction.max_entries", 1024, False),
    ("native-max-construction-dimension", "selection.construction.max_dimension", 20, False),
    ("native-max-construction-table-values", "selection.construction.max_table_values", 1 << 20, False),
    ("native-max-constraint-rows", "selection.construction.max_constraint_rows", 1 << 20, False),
    ("native-max-parity-rows", "selection.construction.max_parity_rows", 1 << 20, False),
    ("native-max-verification-work", "selection.construction.max_verification_work", 16 << 20, True),
    ("native-max-geometry-words", "selection.preparation.max_geometry_words", 16 << 20, False),
    ("native-max-terms", "selection.preparation.max_terms", 256, False),
    ("native-max-core-dimension", "selection.preparation.core.max_dimension", 20, False),
    ("native-max-core-table-values", "selection.preparation.core.max_table_values", 1 << 20, False),
    ("native-max-core-work", "selection.preparation.core.max_work", 64 << 20, True),
    ("injection-max-work", "injection.max_work", 64 << 20, True),
    ("injection-max-geometry-words", "injection.max_geometry_words", 8 << 20, False),
    ("injection-max-terms", "injection.max_terms", 1 << 20, False),
    ("native-transport-max-terms", "transport.max_terms", 256, False),
    ("native-transport-max-geometry-words", "transport.max_geometry_words", 16 << 20, False),
    ("native-transport-max-construction-work", "transport.max_construction_work", 1 << 40, True),
)


def mode(args, family):
    return getattr(args, family + "_preparation", None)


def uses_fibers(args):
    return any(mode(args, family) in ("fixed", "catalogue") for family in FAMILIES)


def add_arguments(parser):
    for family in FAMILIES:
        parser.add_argument("--" + family + "-preparation", choices=MODES)
    parser.add_argument("--native-transport", choices=("off", "prepared"))
    for flag, _, _, _ in LIMITS:
        parser.add_argument("--" + flag, type=int)


def validate(args, parser):
    modes = [mode(args, family) for family in FAMILIES]
    enabled = any(value in ("native", "native_injection") for value in modes)
    if getattr(args, "native_transport", None) is not None and not enabled:
        parser.error("--native-transport requires native or native_injection preparation")
    for flag, _, _, wide in LIMITS:
        value = getattr(args, flag.replace("-", "_"), None)
        if value is None:
            continue
        if type(value) is not int or not 0 <= value <= (2**128 - 1 if wide else 2 * sys.maxsize + 1):
            parser.error(f"--{flag} needs a nonnegative integer in range")
        if not enabled or (flag.startswith("injection-") and "native_injection" not in modes):
            parser.error(f"--{flag} requires native{'_injection' if flag.startswith('injection-') else ' or native_injection'} preparation")
        if flag.startswith("native-transport-") and getattr(args, "native_transport", None) != "prepared":
            parser.error(f"--{flag} requires --native-transport prepared")


def command(args):
    result = []
    for family in FAMILIES:
        if mode(args, family) is not None:
            result += ["--" + family + "-preparation", mode(args, family)]
    transport = getattr(args, "native_transport", None)
    if transport is not None:
        result += ["--native-transport", transport]
    for flag, _, _, _ in LIMITS:
        value = getattr(args, flag.replace("-", "_"), None)
        if value is not None:
            result += ["--" + flag, str(value)]
    return result


def policy(args, selected):
    result = {"mode": selected, "selection": {"construction": {"max_json_bytes": 16 << 20}, "preparation": {"core": {}}},
              "injection": {} if selected == "native_injection" else None,
              "transport": {"mode": getattr(args, "native_transport", None) or "off"}}
    for flag, path, default, wide in LIMITS:
        if path.startswith("injection.") and result["injection"] is None:
            continue
        value = getattr(args, flag.replace("-", "_"), None)
        value = default if value is None else value
        keys = path.split(".")
        target = result
        for key in keys[:-1]:
            target = target[key]
        target[keys[-1]] = str(value) if wide else value
    return result


def resolved(args):
    return {family: policy(args, mode(args, family)) for family in FAMILIES
            if mode(args, family) in ("native", "native_injection")}


def _same(actual, expected):
    # Python's True == 1 must never admit a Boolean configuration or count.
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(_same(actual[k], v) for k, v in expected.items())
    return actual == expected


def verify(actual, args):
    configuration = actual.get("configuration", {})
    for family, expected in resolved(args).items():
        if configuration.get(family + "_preparation") != expected["mode"] or not _same(
                configuration.get(family + "_preparation_options"), expected):
            raise ValueError(f"Rust example did not apply declared {family} native policy and limits")
        cache = actual.get("prepared_" + family + "_cache")
        if not isinstance(cache, dict):
            raise ValueError(f"missing retained {family} native cache")
        routes, transports = cache.get("native_routes"), cache.get("native_transports")
        def counts(value, keys):
            return (isinstance(value, dict) and value.keys() == set(keys)
                    and all(type(v) is int and v >= 0 for v in value.values()))
        if (not counts(routes, ("direct", "product", "walsh", "injected", "injection_budget_skips"))
                or not counts(transports, ("prepared", "budget_skipped"))
                or any(type(cache.get(k)) is not int or cache[k] < 0
                       for k in ("fixed_resources", "cached_queries", "catalogue_resources"))):
            raise ValueError(f"invalid retained {family} native counts")
        total = sum(routes[k] for k in ("direct", "product", "walsh", "injected"))
        if (total != cache["fixed_resources"] or cache["catalogue_resources"] != 0
                or routes["injection_budget_skips"] > total
                or sum(transports.values()) > total):
            raise ValueError(f"inconsistent retained {family} native counts")
        if expected["injection"] is None and (routes["injected"] or routes["injection_budget_skips"]):
            raise ValueError(f"unexpected {family} injection")
        if expected["transport"]["mode"] == "off" and any(transports.values()):
            raise ValueError(f"unexpected prepared {family} transport")
        if expected["transport"]["mode"] == "prepared" and any(
                expected["transport"][key] in (0, "0")
                for key in ("max_terms", "max_geometry_words", "max_construction_work")) and transports["prepared"]:
            raise ValueError(f"zero-budget {family} transport was prepared")
