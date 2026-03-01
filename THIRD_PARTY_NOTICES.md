# Third-Party Notices

## Included third-party material in this repository

### keybr.com

- Upstream project: <https://github.com/aradzie/keybr.com>
- Upstream license: GNU Affero General Public License v3.0
- Local upstream license copy (for local research clone): `clones/keybr.com/LICENSE`

1. `assets/words-en.json`
   - Source: `clones/keybr.com/packages/keybr-content-words/lib/data/words-en.json`
   - Status: included in this repository and used at runtime by `src/generator/dictionary.rs`
   - Modifications: none (byte-identical at the time of import)

## Local research clones (not committed to this repository)

The `clones/` directory is gitignored and used for local research only.

### keybr.com

- License file in local clone: `clones/keybr.com/LICENSE`
- Upstream states AGPLv3 in README/license materials.

### typr

- License file in local clone: `clones/typr/LICENSE`
- License text present in clone is GPLv3.

## Runtime-downloaded content (not version-controlled by default)

This project can download third-party source content at runtime:

- Code snippets from repositories listed in `src/generator/code_syntax.rs`
- Passage text from Project Gutenberg URLs in `src/generator/passage.rs`

Downloaded files are stored in user data directories by default (`dirs::data_dir()`),
not in this repository. These downloaded assets keep their original upstream licenses.

When code snippets are downloaded, keydr now writes a sidecar source manifest
(`*_*.sources.txt`) listing exact source URLs to help with attribution and compliance
if cached content is redistributed.

## Research references from planning docs (idea-only unless noted above)

The following projects are referenced in planning/research docs and were used for
architecture/algorithm ideas:

- keybr.com
- typr
- ttyper
- smassh
- gokeybr
- ivan-volnov/keybr
- keybr-code

For these references, no direct code/data inclusion is claimed in this repository
except the explicitly documented `assets/words-en.json` import from keybr.com.

## Notes

This repository is licensed under AGPL-3.0-only to remain compatible with included
AGPL-licensed upstream material.
