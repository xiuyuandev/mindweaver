# X08

## Scope

Audit model, commands, file format and migration risk before UI expansion.

## Changes

Added `docs/27-X100数据与命令审计.md` with reusable fields, additive style/content requirements,
Workbook risks, command extension order and version-1 compatibility policy.

## Verification

Audit covers `Document`, `Node`, `StyleOverride`, layouts, commands, `.zmind` format version 1,
FileDoc, assets, cards and existing tests.

## Known gaps

Workbook, comments, generic attachments, audio, callout and grouping are intentionally deferred
to their assigned rounds.

