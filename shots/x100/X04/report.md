# X04

## Scope

Produce first-pass paired evidence between Xmind and the current MindWeaver build.

## Changes

Added `scripts/x100-contact-sheet.ps1`, a pair manifest and sixteen triage contact sheets under
`shots/x100/X04/`. Added a physical 1280x800 app capture for shell comparison and a gallery with
state-by-state conclusions in `shots/x100/X04/gallery.md`.

## Verification

Canvas, inspector, node menu, export, presentation, theme and start pairs render correctly. The
final Stage A release binary used for the new captures has SHA256
`64C27A6BFE1B7EE70393FFDDDA62DAFC42D94EBD59F5F861D04628ED87D0C18A`.

Six pairs now use the live Xmind 26.5.1107 corpus instead of legacy screenshots. The captures reveal
that the actual Outliner is more text-first than the old design assumption: a top-center list toolbar
and six-dot drag grip are prominent, while permanent row-level `+ / ⋮` buttons are not shown in the
captured default, edit or multi-select states.

## Known gaps

Reference and app documents are not identical yet, so these are visual triage pairs rather than
pixel gates. The clean blank/root-only Xmind pair remains blocked by X02.

