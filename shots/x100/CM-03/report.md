# CM-03 连线与链接短语 · 报告

> 状态：**已实现**（本包范围；G-CM-1 门未开）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5.4 §5.7 §5.9
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6
> 开始：2026-10-26 · 依赖 CM-01 / CM-02

## 1. 起点（审计结果）

**连线与短语的骨架早就有了**，本包不是从零实现，而是按 §5.4/§5.9 校准：

| 既有能力 | 位置 |
| --- | --- |
| 拖出连线建关系 | `canvas_methods_3.rs::commit_link`（松在空白处不落盘） |
| 短语就地编辑 | `start_rel_edit` / `rel_editor_ui`（`RelEdit` 状态） |
| 改短语 / 改颜色 / 改目标 / 删线 | `Command::{SetRelationshipLabel, SetRelationshipColor, SetRelationshipTarget, DeleteRelationship}` |
| 双击 / 右键菜单进编辑 | `edit_relationship_label_action` |

## 2. 范围（本包改的四件事）

| # | 缺陷 | 处置 |
| --- | --- | --- |
| 1 | 短语编辑框有 `panel` 底 + 强调色描边 + 6px 内距，看起来像画布上飘着一个控件 | 去掉描边与内距，只留 `canvas_bg` 底 + 4/2 内距；字号改 `design::content::LIST_META.size`（11.5），与胶囊字同源 |
| 2 | **IME 组合中按 Enter 会误提交**：`rel_editor_ui` 里 `enter` 无条件为真，中文输入法选字那一下就把半截拼音写进短语 | 新增 `RelEdit.ime`，每帧用 `ime_transition(&events)` 更新；`enter = !ed.ime && consume_key(Enter)` |
| 3 | 空短语什么都不画：新拖出来的线是一根没头没脑的箭头 | 概念图空短语在短语位置画 `＋ 链接词`（12px `text.dim` 居中），提示「这条线还差一句话」 |
| 4 | `§5.9「未填不给连线落盘」` 被绕过：拖出即落盘，用户 Esc 后留下一条无词线 | 概念图拖出后**立刻进短语编辑态**；`RelEdit.blank_new = true`，留空 Esc（或失焦仍为空）→ `Command::DeleteRelationship` 把线一起撤掉 |

## 3. 非目标

- 不做交叉处的 ≤8px 微弧（§5.4 最后一条）：需要线的交叉检测 + 分段改道，属渲染层独立特性，留 CM-05。
- 不做连线的拖动改目标（`RelinkDrag` 已有，未按概念图调）。
- 不做卡片联动（CM-06）、大纲（CM-07）。

## 4. 验证

| 项 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 新增 0 |
| `cargo test --workspace -- --test-threads=1` | **790 passed / 0 failed** |
| 机器门（33 场景双跑 `--tol 0`） | 见下（`target/x100/stage-b-gate.log`） |
| 反例 | 中途被 `chrome::tests::shell_sources_do_not_hardcode_font_sizes` 拦下一次——我写了 `FontId::proportional(11.5)`；改成 `design::content::LIST_META.size` 后过。这条断言是有用的。 |

## 5. 未做 / 偏差（必须留在纸面上）

1. **没有交互脚本场景**：`cm-link-create` / `cm-label-ime` 两张场景未拍。原因：拖拽脚本要按相机算世界→屏幕坐标，
   而概念图的相机缩放随包围盒变化；先用人工验证（下条），脚本化留在 CM-05 与 `cm-*` 证据包（CM-08）一起做。
2. **IME 守卫只做了逻辑，没有真实输入法实测**：`RelEdit.ime` 的流转与节点编辑同源
   （共用 `ime_transition`），但中文输入法「选字不回写」这一条还需要用户在真机上敲一次才算验证。
   HANDOFF 里「美敲拼音人工实测待用户配合」是同一笔账。
3. **空短语占位不参与命中**：点「＋ 链接词」本身不会进编辑（要先点线选中再双击/回车）。
   属交互细节，随 CM-05 统一处理。
4. **思维导图仍允许空短语落盘**：`commit_link` 只在 `kind == "conceptMap"` 时进编辑 + 回滚，
   思维导图保持既有行为（那里的关系线是辅助标注，不是命题）。
