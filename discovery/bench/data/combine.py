# Combines the runs' markdown tables: for each row, the min of the mins and the
# median of the medians. Usage: python3 -I combine.py target/node-run*.md
import sys, statistics
rows, order, headers = {}, [], {}
for path in sys.argv[1:]:
    head = None
    for line in open(path):
        line = line.rstrip("\n")
        if line.startswith("<!--"):
            continue
        if not line.startswith("|") or line.startswith("|---"):
            head = None if not line.startswith("|") else head
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if head is None or cells[0] in ("path", "what", "downsampler", "downsampler (wasm)", "body") or cells[0].startswith("ring append"):
            head = cells
            headers.setdefault(tuple(cells), None)
            continue
        key = (tuple(head), cells[0], cells[1])
        if key not in rows:
            rows[key] = []
            order.append(key)
        rows[key].append(cells)
last = None
for key in order:
    head = key[0]
    if head != last:
        print("\n| " + " | ".join(head) + " | runs |\n|" + "---|" * (len(head) + 1))
        last = head
    runs = rows[key]
    out = list(runs[0])
    for i, h in enumerate(head):
        try:
            vals = [float(r[i]) for r in runs]
        except ValueError:
            continue
        if h.startswith("min"):
            out[i] = f"{min(vals):.3g}"
        elif "median" in h:
            out[i] = f"{statistics.median(vals):.3g}"
    print("| " + " | ".join(out) + f" | {len(runs)} |")
