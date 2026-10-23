"""The Arrow IPC stream fixtures of the core's Arrow reader (REQ-DATA-140).

Writes small `.arrows` streams with pyarrow into
crates/glyphplot/tests/fixtures/arrow/, and beside them `expected.json`: for
each stream that should read, its row count and every column's name, type,
time unit, zone and values as pyarrow gives them (timestamps as integers in
their unit, floats as Python's shortest round-trip text, dictionary columns
as their labels, null as JSON null). The streams that should not read (compressed, an unsupported type)
are listed with no expectation; the Rust tests name the error.

Run with pyarrow pinned, from the repository root:

    mise exec -- uv run --no-project --with 'pyarrow==26.0.0' python -I scripts/arrow-fixtures.py

`--check` writes nothing and exits 1 when a file would change.
"""

import io
import json
import pathlib
import sys

import pyarrow as pa

OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/glyphplot/tests/fixtures/arrow"

# 2^53 and the ns instants of REQ-DATA-020, past 2^53 ns.
P53 = 2**53
NS = [1792281600000000001, 1792281600000000002, None, 1792281600000000004]


def stream(schema, batches, **options):
    """An IPC stream of `batches` under `schema`, written with `options`."""
    sink = io.BytesIO()
    with pa.ipc.new_stream(sink, schema, options=pa.ipc.IpcWriteOptions(**options)) as w:
        for b in batches:
            w.write_batch(b)
    return sink.getvalue()


def dictionary(values, labels, index=pa.int32()):
    """A dictionary array of `labels` indexed by `values` (None is null)."""
    return pa.DictionaryArray.from_arrays(pa.array(values, type=index), pa.array(labels, type=pa.string()))


def types():
    """One batch, every type the reader takes, a null in every column."""
    cols = {
        "f64": pa.array([1.5, None, -0.25, 1e300, 0.0, -1e-300], pa.float64()),
        "f32": pa.array([1.5, -2.75, None, 3.4028234663852886e38, 1e-45, 0.1], pa.float32()),
        "i32": pa.array([-(2**31), 2**31 - 1, None, 0, -1, 7], pa.int32()),
        "i64": pa.array([-P53, P53, -1, None, 0, 123456789012345], pa.int64()),
        "u32": pa.array([0, 2**32 - 1, 1, None, 65536, 9], pa.uint32()),
        "i8": pa.array([-128, 127, None, 0, 1, -1], pa.int8()),
        "i16": pa.array([-32768, 32767, None, 0, 1, -1], pa.int16()),
        "u8": pa.array([0, 255, None, 1, 2, 3], pa.uint8()),
        "u16": pa.array([0, 65535, None, 1, 2, 3], pa.uint16()),
        "flag": pa.array([True, False, None, True, True, False], pa.bool_()),
        "name": pa.array(["alpha", "", None, "ünïcode ✓", "x" * 40, "z"], pa.string()),
        "big_name": pa.array(["a", None, "bb", "ccc", "", "d"], pa.large_string()),
        "kind": dictionary([0, 1, None, 1, 2, 0], ["low", "mid", "high"]),
        "kind8": dictionary([2, None, 0, 0, 1, 2], ["b", "a", "c"], pa.int8()),
        "t_s": pa.array([0, 1792281600, None, -86400, 1, 2], pa.timestamp("s")),
        "t_ms": pa.array([1792281600123, None, 0, -1, 5, 6], pa.timestamp("ms", tz="America/New_York")),
        "t_us": pa.array([1792281600000001, 1792281600000002, None, 1792281600000000, 0, -1], pa.timestamp("us", tz="UTC")),
        "t_ns": pa.array([1792281600000000001, None, 1792281600000000003, 1792281600000000004, 1792281600000000005, 1792281600000000006], pa.timestamp("ns", tz="+09:00")),
    }
    batch = pa.record_batch(list(cols.values()), names=list(cols))
    return stream(batch.schema, [batch])


def batches():
    """Several batches, one empty and one sliced, with delta dictionaries."""
    schema = pa.schema([("x", pa.float64()), ("y", pa.int64()), ("series", pa.dictionary(pa.int32(), pa.string()))])
    labels = ["a", "b", "c", "d", "e"]

    def batch(x, y, codes, n):
        return pa.record_batch([pa.array(x, pa.float64()), pa.array(y, pa.int64()), dictionary(codes, labels[:n])], schema=schema)

    big = batch([float(i) for i in range(20)], list(range(100, 120)), [i % 3 if i % 5 else None for i in range(20)], 3)
    out = [
        batch([0.0, 1.0], [10, None], [0, 1], 2),
        batch([], [], [], 2),
        batch([2.0, None, 4.0], [12, 13, 14], [1, 2, None], 3),
        big.slice(3, 11),
        batch([30.0], [-5], [4], 5),
    ]
    return stream(schema, out, emit_dictionary_deltas=True)


def replaced():
    """Two batches whose dictionaries differ: a replacement, not a delta."""
    schema = pa.schema([("x", pa.float64()), ("kind", pa.dictionary(pa.int32(), pa.string()))])
    one = pa.record_batch([pa.array([0.0, 1.0, 2.0]), dictionary([0, 1, 0], ["red", "green"])], schema=schema)
    two = pa.record_batch([pa.array([3.0, 4.0, 5.0]), dictionary([0, 1, None], ["blue", "red"])], schema=schema)
    return stream(schema, [one, two], emit_dictionary_deltas=False)


def nanoseconds():
    """ns instants past 2^53 and int64 values at ±2^53 (REQ-DATA-020)."""
    batch = pa.record_batch(
        [pa.array(NS, pa.timestamp("ns")), pa.array([-P53, P53, None, P53 - 1], pa.int64()), pa.array([1.0, 2.0, 3.0, None])],
        names=["t", "n", "y"],
    )
    return stream(batch.schema, [batch])


def empty():
    """A schema and no batch: a table of no rows."""
    schema = pa.schema([("t", pa.timestamp("ms")), ("y", pa.float64()), ("kind", pa.dictionary(pa.int32(), pa.string()))])
    return stream(schema, [])


def legacy():
    """The pre-0.15 framing, no continuation marker, metadata V4."""
    batch = pa.record_batch([pa.array([1, 2, None], pa.int32()), pa.array([0.5, None, 2.5])], names=["x", "y"])
    return stream(batch.schema, [batch], use_legacy_format=True, metadata_version=pa.ipc.MetadataVersion.V4)


def compressed():
    """A zstd-compressed batch: the reader refuses it."""
    batch = pa.record_batch([pa.array([1.0, 2.0, 3.0])], names=["y"])
    return stream(batch.schema, [batch], compression="zstd")


def unsupported():
    """A date32 column: the reader refuses it, naming the column."""
    batch = pa.record_batch([pa.array([1.0, 2.0]), pa.array([0, 1], pa.date32())], names=["y", "day"])
    return stream(batch.schema, [batch])


READ = {"types": types, "batches": batches, "replaced": replaced, "ns": nanoseconds, "empty": empty, "legacy": legacy}
REFUSED = {"compressed": compressed, "unsupported": unsupported}


def expect(data):
    """What the reader must give for the stream `data`, read back by pyarrow."""
    table = pa.ipc.open_stream(data).read_all()
    columns = []
    for field, chunked in zip(table.schema, table.columns):
        t = field.type
        col = {"name": field.name, "type": str(t)}
        if pa.types.is_timestamp(t):
            col["unit"] = t.unit
            col["zone"] = t.tz
            col["values"] = chunked.cast(pa.int64()).to_pylist()
        elif pa.types.is_dictionary(t):
            col["values"] = chunked.cast(t.value_type).to_pylist()
        elif pa.types.is_floating(t):
            # As text: a JSON reader need not round a float's digits correctly.
            col["values"] = [None if v is None else repr(v) for v in chunked.to_pylist()]
        else:
            col["values"] = chunked.to_pylist()
        columns.append(col)
    return {"rows": table.num_rows, "columns": columns}


def main():
    check = "--check" in sys.argv[1:]
    files = {}
    expected = {"pyarrow": pa.__version__, "read": {}, "refused": sorted(REFUSED)}
    for name, make in READ.items():
        files[f"{name}.arrows"] = make()
        expected["read"][name] = expect(files[f"{name}.arrows"])
    for name, make in REFUSED.items():
        files[f"{name}.arrows"] = make()
    files["expected.json"] = (json.dumps(expected, indent=1, ensure_ascii=False) + "\n").encode()

    changed = [n for n, b in files.items() if not (OUT / n).exists() or (OUT / n).read_bytes() != b]
    if check:
        for n in changed:
            print(f"arrow-fixtures: {n} would change")
        sys.exit(1 if changed else 0)
    OUT.mkdir(parents=True, exist_ok=True)
    for n, b in files.items():
        (OUT / n).write_bytes(b)
    print(f"arrow-fixtures: {len(files)} files in {OUT.relative_to(OUT.parents[4])}, {len(changed)} changed, pyarrow {pa.__version__}")


if __name__ == "__main__":
    main()
