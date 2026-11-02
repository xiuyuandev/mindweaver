# X10

## Scope

Publish the first ten rounds and stop at the mandatory human gate.

## Changes

Added the canonical X100 roadmap and stage-A report; linked the plan from the repository entry
documents; old P/V/O/R/WP plans are retained as history only.

## Verification

Technical checks are recorded in `docs/x100/stage-a-report.md`. The gate remains
`awaiting_user` because the plan requires human review after every ten rounds.
`scripts/x100-stage-a-gate.ps1` now reproduces the machine portion in one command and writes
`target/x100/stage-a-gate.log`. The full run log is persisted in
`shots/x100/X10/traces/stage-a-gate.log`.

## Known gaps

X02 is resolved. Eleven clean live Xmind states were captured with a temporary profile, bringing the
reference index to 73 images and the contact-sheet count to 17. Remaining gaps are mouse-down drag
preview, dark appearance and matched DPI pairs. The X10 human gate is still `awaiting_user`, so X11
must not start.

