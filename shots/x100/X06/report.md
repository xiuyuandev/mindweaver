# X06

## Scope

Replace checkbox-only completion with evidence-driven status.

## Changes

Added `docs/26-X100完成定义与验收规范.md`, evidence directory schema, visual/behavior thresholds,
retry rules and stage-gate requirements.

## Verification

The evidence checker now rejects invalid status values, missing manifest fields, missing paths and
verified rounds containing non-pass scenarios.

## Known gaps

Human visual acceptance cannot be automated; it remains a mandatory stage gate.

