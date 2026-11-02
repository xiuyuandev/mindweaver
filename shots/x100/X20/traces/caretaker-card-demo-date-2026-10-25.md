# X20 门后维护 ⑫ · 卡片演示日期冻结（2026-10-25）

范围：`crates/mw-shell/src/app_methods_1.rs`、`app_tests.rs`。只改
`MW_CARD_DEMO` 的演示种子与一条单测；不改真实笔记的 `now_ms()` /
`NoteCard`，不重拍基线，不进入 Stage C。

## 缺口

`MW_CARD_DEMO` 建示例卡片时用 `mw_core::now_ms()` 作为时间戳，卡片阅读页
渲染「创建于 / 更新于」，`format_date` 按 UTC 取日。本地时间 08:00
（UTC+8）跨日后，同一份代码渲染出的日期从 `2026-10-24` 变 `2026-10-25`，
而 `card-slash` 基线是 2026-10-24 22:30 生成的。

2026-10-25 08:27 的 ⑪ 阶段门首次暴露（07:52 的同一条门仍 62/62，因为当时
UTC 还是 10-24）：

```
[golden] 198/1749600 像素超容差（0.011%），最大差 92，
         包围盒 x[1380..1390] y[370..415]
FAIL golden card-slash
MACHINE GATE FAIL: golden matrix pass 1 …
```

两遍门完全一致（不是抖动）。差异位置正是阅读页底部的两行日期：
基线 `2026-10-24`，实跑 `2026-10-25`
（`caretaker-card-demo-date-context-baseline-2026-10-25.png`、
`…-baseline-crop…png` / `…-drift-crop…png`）。这是既有缺陷，与 ⑪ 的
大纲键盘改动无关。

## 修复

- `app_methods_1.rs`：`DEMO_CARD_TS_MS = 1_792_800_000_000`
  （2026-10-24 00:00 UTC），`MW_CARD_DEMO` 的 `NoteCard::new` 改用该常量；
  日期与既有基线一致，之后任何 UTC 跨日都不再漂。
- `app_tests.rs`：`card_demo_timestamp_is_frozen_for_golden` 断言
  `format_date(DEMO_CARD_TS_MS) == "2026-10-24"`，防止有人改回墙钟。
- 真实笔记（新建卡片、保存、排序）仍用 `now_ms()`，行为不变。

## 证据

- 修复前失败门：`caretaker-stage-b-gate-card-date-fail-2026-10-25.log`
  （started 08:27:33，finished 08:28:58，card-slash 两遍均 198 px）。
- 修复后单跑 `card-slash`（`MW_CARD_DEMO=1 MW_CARD_SLASH=1`、ppp 1.5、
  warmup 3、`--tol 0`）：

  ```
  [golden] ✓ 无超容差差异（1620×1080，容差 0）
  exit=0
  ```

  基线 `shots/golden/card-slash/shell-ppp1.50.png` 未被改写。
- 全量测试 **763 passed / 0 failed**（新增 1 条），
  `caretaker-outline-keys-tests-2026-10-25.txt`。
- 阶段门 31 组 × 2 次 `--tol 0` **62/62**、fmt 干净、mw-shell clippy 仅
  `export_pdf.rs` 3 条历史，结论 `MACHINE GATE PASS; HUMAN X20 REVIEW IS
  STILL REQUIRED`；日志
  `caretaker-stage-b-gate-card-date-2026-10-25.log`。
- release 23,469,056 B / SHA256 `a4662b1f…f5babbb862`。
