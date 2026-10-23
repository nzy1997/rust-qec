"""Explicit insertion-route options and independent receipt validation."""


def add_arguments(parser):
    parser.add_argument("--insertion-preparation", choices=["direct", "prepared"], default="direct")
    parser.add_argument("--max-insertion-table-bytes", type=int)
    parser.add_argument("--max-insertion-catalog-entries", type=int)


def validate(args, parser):
    limits = (args.max_insertion_table_bytes, args.max_insertion_catalog_entries)
    if any(value is not None and value < 0 for value in limits):
        parser.error("insertion limits must be nonnegative")
    if args.insertion_preparation == "direct" and any(value is not None for value in limits):
        parser.error("insertion limits require prepared insertion mode")


def resolved(args):
    prepared = args.insertion_preparation == "prepared"
    return dict(
        insertion_preparation=args.insertion_preparation,
        max_insertion_table_bytes=(64 * 1024**2 if args.max_insertion_table_bytes is None else args.max_insertion_table_bytes) if prepared else None,
        max_insertion_catalog_entries=(1_000_000 if args.max_insertion_catalog_entries is None else args.max_insertion_catalog_entries) if prepared else None,
    )


def command(args):
    result = ["--insertion-preparation", args.insertion_preparation]
    if args.insertion_preparation == "prepared":
        values = resolved(args)
        result += ["--max-insertion-table-bytes", str(values["max_insertion_table_bytes"]),
                   "--max-insertion-catalog-entries", str(values["max_insertion_catalog_entries"])]
    return result


def verify(actual, args):
    configuration = actual.get("configuration", {})
    for name, expected in resolved(args).items():
        value = configuration.get(name)
        if name not in configuration or type(value) is not type(expected) or value != expected:
            raise RuntimeError(f"Rust example did not apply declared {name}")
    catalog = actual.get("prepared_insertions")
    if args.insertion_preparation == "direct":
        if catalog is not None:
            raise RuntimeError("direct insertion mode unexpectedly retained a catalog")
        return
    # These comparison drivers request the fixed-template route, which retains
    # its catalog. An unrequested cold-prefix route cannot excuse a missing one.
    if not isinstance(catalog, dict):
        raise RuntimeError("prepared insertion mode did not retain a reference catalog")
    entries, size = catalog.get("catalog_entries"), catalog.get("estimated_peak_bytes")
    limits = resolved(args)
    if type(entries) is not int or type(size) is not int or not 0 <= entries <= limits["max_insertion_catalog_entries"] or not 0 <= size <= limits["max_insertion_table_bytes"]:
        raise RuntimeError("insertion catalog diagnostics violate declared limits")
    if (entries == 0) != (size == 0):
        raise RuntimeError("empty insertion catalog and zero footprint disagree")
