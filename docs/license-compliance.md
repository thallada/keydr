# License Compliance Notes

This repository includes AGPL-licensed upstream material and is licensed as
`AGPL-3.0-only`.

## What is included in-repo

- `assets/words-en.json` is imported from keybr.com and tracked in
  `THIRD_PARTY_NOTICES.md`.
- `assets/words-en.json.license` records source and license for the imported file.

## What is research-only

- The `clones/` directory is gitignored and used for local analysis only.
- References in `docs/plans/` to third-party projects are primarily idea-level
  research unless explicitly documented as imported content.

## Runtime downloads

- Code drills can download source files from upstream GitHub repositories.
- Passage drills can download texts from Project Gutenberg.
- Downloaded content is cached in user data directories by default, not in this repo.
- Downloaded code snippet caches include a `*.sources.txt` sidecar with source URLs.

## Ongoing checklist for compliance

1. If you import any third-party file into the repository, add it to
   `THIRD_PARTY_NOTICES.md`.
2. Add a sidecar `filename.license` (or equivalent) with source and license.
3. Keep the project license compatible with imported copyleft obligations.
4. If you later commit downloaded snippet caches or passage corpora, include
   attribution and the relevant upstream license terms for those files.
