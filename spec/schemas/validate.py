# Checks spec/schemas: every *.schema.json is a valid draft 2020-12 schema
# whose $id names its file and whose every property has a description;
# $refs between files resolve by $id through a `referencing` Registry; every
# examples/<name>[.<what>].json validates against <name>.schema.json (formats
# checked); every examples/invalid/<name>.<what>.json fails it; and every
# schema has at least one example of each kind.
#
# Run from the repository root:
#   mise exec -- uv run --no-project --with 'jsonschema[format]==4.25.1' --with referencing python -I spec/schemas/validate.py
import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

BASE = "https://github.com/curtisault/glyphplot/spec/schemas/"
DIR = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd() / "spec" / "schemas"


def load(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def undescribed(node, where, skip=False):
    """Yield paths of entries under `properties` that have no description."""
    if isinstance(node, dict):
        for key, value in node.items():
            here = f"{where}/{key}"
            inner_skip = skip or key in ("if", "then", "else")
            if key == "properties" and isinstance(value, dict) and not skip:
                for name, sub in value.items():
                    if isinstance(sub, dict) and "description" not in sub:
                        yield f"{here}/{name}"
            yield from undescribed(value, here, inner_skip)
    elif isinstance(node, list):
        for i, value in enumerate(node):
            yield from undescribed(value, f"{where}/{i}", skip)


def main():
    failures = []
    schemas = {}
    for path in sorted(DIR.glob("*.schema.json")):
        name = path.name[: -len(".schema.json")]
        schema = load(path)
        schemas[name] = schema
        try:
            Draft202012Validator.check_schema(schema)
        except Exception as e:  # noqa: BLE001
            failures.append(f"{path.name}: not a valid schema: {e.message}")
        if schema.get("$id") != BASE + path.name:
            failures.append(f"{path.name}: $id is {schema.get('$id')!r}, expected {BASE + path.name!r}")
        for where in undescribed(schema, "#"):
            failures.append(f"{path.name}: {where} has no description")

    registry = Registry().with_resources(
        (s["$id"], Resource.from_contents(s)) for s in schemas.values()
    )
    validators = {
        name: Draft202012Validator(
            s, registry=registry, format_checker=Draft202012Validator.FORMAT_CHECKER
        )
        for name, s in schemas.items()
    }

    counts = {name: [0, 0] for name in schemas}
    valid = sorted((DIR / "examples").glob("*.json"))
    invalid = sorted((DIR / "examples" / "invalid").glob("*.json"))
    for path, should_pass in [(p, True) for p in valid] + [(p, False) for p in invalid]:
        name = path.name.split(".")[0]
        rel = path.relative_to(DIR)
        if name not in validators:
            failures.append(f"{rel}: no schema named {name}.schema.json")
            continue
        try:
            errors = list(validators[name].iter_errors(load(path)))
        except Exception as e:  # noqa: BLE001  (an unresolvable $ref, bad JSON)
            failures.append(f"{rel}: {type(e).__name__}: {e}")
            continue
        if should_pass:
            counts[name][0] += 1
            for err in errors[:5]:
                failures.append(f"{rel}: {'/'.join(map(str, err.absolute_path)) or '(root)'}: {err.message}")
        else:
            counts[name][1] += 1
            if not errors:
                failures.append(f"{rel}: validated, but it must fail")

    for name, (ok, bad) in counts.items():
        if ok == 0:
            failures.append(f"{name}.schema.json: no valid example")
        if bad == 0:
            failures.append(f"{name}.schema.json: no invalid example")

    for name, (ok, bad) in counts.items():
        print(f"{name + '.schema.json':24} {ok:2} valid  {bad:2} invalid")
    print(f"{len(schemas)} schemas, {len(valid)} valid examples, {len(invalid)} invalid examples")
    if failures:
        print(f"\n{len(failures)} failure(s):")
        for f in failures:
            print("  " + f)
        sys.exit(1)
    print("all ok")


main()
