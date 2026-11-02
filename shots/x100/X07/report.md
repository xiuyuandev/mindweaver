# X07

## Scope

Strengthen deterministic input capture for hover, scroll and script lifecycle evidence.

## Changes

`MW_GOLDEN_INPUT` now supports `hover x y [hold_ms]`, `scroll dx dy`, and emits
`golden_input_start` / `golden_input_done` trace events. Added a unit test for both actions.

## Verification

`cargo test -p mw-shell golden_input::tests` passes 5/5. The 13-step smoke script produced a
1620x1080 screenshot and a trace containing hover hold, wheel delta and start/done markers. A
second run at ppp 1.50 / tolerance 0 reported no differences.

## Known gaps

Menu and IME paths still need dedicated matrix scenarios in later rounds.

