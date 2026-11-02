# X100 Stage A gate report

> Prepared: 2026-10-22 01:20 CST · Reference: Xmind `26.5.1107.202609082009`
>
> Gate state: **accepted by the user on 2026-10-24** ("可以了，推进开发吧"). Stage B
> (`X11–X20`) may proceed; the P0 order in `docs/25` remains the working priority.
>
> Execution and retry timeline: [`night-run-log.md`](night-run-log.md).
> Repeated external-blocker audit: [`stage-a-blocked-audit.md`](stage-a-blocked-audit.md).

## 1. Round status

| Round | Status | Evidence |
| --- | --- | --- |
| X01 | verified | `docs/24`, `docs/25`, `docs/27` |
| X02 | verified | safe refusal with 9 user processes, then isolated launch/capture/close with 0 before and after |
| X03 | implemented | 15 scenario groups, 73-image metadata index and 11 clean live state captures |
| X04 | implemented | sixteen contact sheets, gallery and a physical 1280x800 capture under `shots/x100/X04` |
| X05 | verified | prioritized gap matrix in `docs/25` |
| X06 | verified | evidence rules in `docs/26` |
| X07 | verified | hover/scroll input and double-run golden under `shots/x100/X07` |
| X08 | verified | model/command/format audit in `docs/27` |
| X09 | verified | 11-node, 126-node and deterministic 20k fixtures |
| X10 | in progress | this gate report |

## 2. Technical evidence

- Release build: `target/release/mw-shell.exe`, 23,272,448 bytes, SHA256
  `64C27A6BFE1B7EE70393FFDDDA62DAFC42D94EBD59F5F861D04628ED87D0C18A`.
- `MW_GOLDEN_INPUT` smoke test: 13 steps, hover held 700 ms, wheel delta `(0, -240)`.
- Script trace includes `golden_input_start` and `golden_input_done`.
- Golden rerun at ppp 1.50 and tolerance 0: no differences, 1620x1080.
- Fixture test: tiny and complex files load, validate, save and reload; truncated bytes are rejected.
- Performance corpus: 20,000 nodes, 423,341 bytes, deterministic four-way tree.
- Full serial workspace test after Stage A: **681 passed / 0 failed**.
- `cargo fmt --all -- --check` clean; new X100 code adds zero clippy warnings. Only historical
  warnings remain in `mw-core`, `mw-render` and the three known `export_pdf.rs` sites.
- Default shell golden was rerun twice at tolerance 0; both runs reported no differences.
- One-command machine gate passed end-to-end: `MACHINE GATE PASS; HUMAN X10 REVIEW IS STILL
  REQUIRED`. Latest log: `target/x100/stage-a-gate.log`.
- The X02 blocker was resolved after the user closed Xmind. A repository-local temporary profile
  captured 11 clean states; login and personal files were avoided, telemetry was disabled, and the
  process count returned to zero.
- Latest reference index: 73 images. Latest contact-sheet count: 17. Both counts pass the machine
  gate; no Xmind process remained after the final check.

## 3. Evidence interpretation

This stage established a trustworthy audit and test foundation. It did **not** implement the
visual redesign. The current side-by-side sheets clearly show the expected P0 gaps:

- MindWeaver keeps a conventional text menu/tool row where Xmind uses a quieter floating control
  cluster.
- Xmind presents a selected-topic toolbar and a dense contextual inspector; MindWeaver does not.
- MindWeaver node cards and branch colors are more visually active and less internally consistent.
- Xmind's inspector uses compact grouped rows; MindWeaver still reserves large empty areas.
- Existing outline captures do not prove Xmind-style hover actions, multi-select drag and row menus.
- Export, presentation and theme flows expose different control density and hierarchy, not merely a
  different accent color.

## 4. Honest blockers

1. X02 is resolved: nine running processes caused a safe refusal; after they closed, an isolated
   temporary profile launched successfully and was stopped after capture.
2. X03 now includes clean home, canvas, menu, Outliner, multi-select, inline editing, navigation
   search, export and presentation captures.
3. X03 still lacks mouse-down drag insertion previews, dark appearance and a matched DPI matrix.
4. X04 has expanded to sixteen triage sheets. Reference and app documents still differ, so these are
   design-triage sheets rather than final pixel gates.

The isolation blocker no longer prevents design work. The remaining reference gaps are explicitly
listed and should be either captured later or accepted as deferred at the X10 gate.

## 5. Gate checklist for user

- Accept the diagnostic conclusion that the project needs a presentation-layer rebuild rather than
  more breadth.
- Approve the P0 order in `docs/25`.
- Confirm that core Xmind parity remains ahead of card/review/knowledge-view polish.
- After closing the existing Xmind instance, either run the isolation workflow or keep those missing
  reference states explicitly deferred.
- Only after this gate should Stage B (`X11–X20`) begin.

Latest gate run: 2026-10-22 06:40–06:41 CST, machine result PASS. Human approval was given on
2026-10-24; Stage B is released to start at X11.

## 6. Commands

```powershell
powershell -File scripts/x100-xmind-capture.ps1 -Mode check
powershell -File scripts/x100-fixtures.ps1
powershell -File scripts/x100-stage-a-gate.ps1
cargo test -p mw-format --test x100_fixtures -- --test-threads=1
cargo test -p mw-shell golden_input::tests -- --test-threads=1
```

