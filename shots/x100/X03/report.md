# X03

## Scope

Catalog the existing reference corpus and state what can be compared today.

## Changes

Added `shots/reference/xmind-26.5.1107/manifest.json` with fifteen scenario groups, viewport and
confidence metadata, plus a precise list of missing states. Added
`scripts/x100-reference-index.ps1` and `corpus.json`; the index records path, dimensions, bytes,
SHA256 and modified time for legacy and live screenshots.

## Verification

Both JSON files parse successfully. Existing references cover canvas, selection toolbar, context menu,
style/shape/structure/theme inspectors, export and a partial presentation state.

The blocker was later resolved. A clean isolated profile produced eleven new captures at native
window sizes: home, default canvas, node context menu, default/multi-select/edit Outliner, navigation
search, presentation and export menu. All login and personal documents were avoided, telemetry was
disabled, and no Xmind process remained after shutdown.

## Known gaps

Mouse-down drag insertion previews, dark appearance, advanced media states and the full DPI matrix
remain uncaptured. The remaining gaps are explicit rather than inferred from old evidence.

