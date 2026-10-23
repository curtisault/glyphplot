# Datasets

`manifest.json` names every dataset the repository commits or the site
serves (REQ-DATASET-010, [ADR-137](../spec/adr/137-dataset-manifest-and-allow-list.md)).
`mise run datasets:check` checks it; `mise run datasets:fetch` fetches what is
fetched at build time. Both are in `scripts/datasets.mjs`, with its tests in
`scripts/datasets.test.mjs`.

## Where data lives

| Folder | What | Committed |
|---|---|---|
| `datasets/` | Data the site and the examples share, beside this file | yes |
| `site/public/data/` | The site's data folder: Vite serves it at `data/` and copies it into `site/dist/data/` | only what is not gitignored |
| `site/public/data/hyg/` | HYG's naked-eye subset and its licence notice, written by `datasets:fetch` | no (gitignored) |
| `crates/glyphplot/tests/data/` | The crate's test datasets, listed in their own README (REQ-FIX-020) | yes |
| `target/datasets/` | Downloads `datasets:fetch` keeps, such as HYG's full release | no |

Every file in the first four folders is a dataset with an entry, except
`manifest.json`, `README.md`, `LICENSE*` and `NOTICE*`.

## An entry

| Field | What |
|---|---|
| `id` | Unique; what `site/examples/manifest.json` names in an example's `datasets` |
| `title` | What the data is and its span |
| `publisher` | Who publishes it |
| `url` | The publisher's page or download URL |
| `licence` | The licence's name, read against the allow-list (below) |
| `licenceUrl` | The licence or the publisher's terms |
| `attribution` | Optional: the credit line a page shows |
| `retrieved` | `YYYY-MM-DD` the bytes were fetched; for `live` and `generated`, the date the URL and terms were recorded |
| `storage` | `committed` (in git), `build` (fetched or derived at build time into a gitignored path), `live` (the reader's browser fetches it from the publisher; no file), `generated` (made by code from a seed; no file) |
| `paths` | `committed` and `build`: every path from the repository root holding these exact bytes |
| `size`, `sha256` | `committed` and `build`: the bytes' length and SHA-256; null or absent for `live` and `generated` |
| `fetch` | `build`: the URL `datasets:fetch` downloads, pinned to a release or commit |
| `derivedFrom`, `transform` | `build`: the entry it is made from, and how (`TRANSFORMS` in `scripts/datasets.mjs`) |
| `notice` | Optional: a licence notice `datasets:fetch` writes beside the data |
| `note` | Optional; required for a `live` feed whose licence is not on the allow-list |

To add a committed file, put it in one of the folders above and run
`node scripts/datasets.mjs hash <file>`: it prints an entry with `paths`,
`size` and `sha256` filled in, to complete and add to `manifest.json`.

## What the check refuses

- A file in a data folder with no entry, or whose size or SHA-256 differs
  from its entry's (REQ-DATASET-010).
- A committed dataset over 1 MB, taken as 1,000,000 B (REQ-DATASET-020).
- A licence that is NonCommercial, NoDerivatives or proprietary, whatever the
  storage. A committed dataset's licence must be on the allow-list: public
  domain, CC0, CC BY, ODC-By, MIT, BSD (REQ-DATASET-020).
- A ShareAlike dataset committed, or in a published crate (`cargo package
  --list` of each crate without `publish = false`) or the npm package (`npm
  pack --dry-run --json` of `web/`), matched by path or by SHA-256
  (REQ-DATASET-030).
- A `build` path that is not gitignored or is tracked.
- `crates/glyphplot/tests/data/README.md` disagreeing with the manifest on a
  file, its source URL, its retrieval date or its licence.
