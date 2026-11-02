# X02

## Scope

Provide a safe, isolated Xmind screenshot workflow without touching the user's running app.

## Changes

Added `scripts/x100-xmind-capture.ps1` with `check`, `launch` and `capture` modes. Launch is
refused whenever any Xmind process is already running; captures are full-window read-only grabs
with a JSON sidecar and executable hash.

## Verification

`-Mode check` first found nine existing Xmind processes and set `launchBlocked=true`. A second
check invoked `-Mode launch` and confirmed it exits with code 1 before starting anything. After the
user processes were closed, the same check reported zero and the isolated launch created PID 2748
with a repository-local profile. No account login or personal document was used. Program-log upload
was unchecked. The isolated process was stopped after capture and the final Xmind process count was
zero.

## Known gaps

Resolved. The workflow must still refuse to run whenever any unrelated Xmind process is present.

