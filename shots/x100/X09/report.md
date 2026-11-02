# X09

## Scope

Create stable small, complex and performance fixtures for later rounds.

## Changes

Added `crates/mw-format/examples/x100_fixtures.rs`, `scripts/x100-fixtures.ps1`, two committed
`.zmind` fixtures and a deterministic on-demand 20k corpus.

## Verification

Tiny fixture: 11 nodes. Complex fixture: 126 nodes. Performance fixture: 20,000 nodes, 423,341
bytes. `x100_fixtures` integration tests pass load, structural validation, save/load roundtrip and
truncated-file rejection.

## Known gaps

The 20k binary remains under `target/`; its hash and generator are recorded in the fixture manifest.

