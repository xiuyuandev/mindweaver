# CM-11 · 概念图复制语义与选中反馈

> 建立：2026-10-30 · 状态：已实现（机器门绿；未记 `已验证`）
> 触发：用户要求完善概念图编辑操作，并保持独立概念、自由布局与数据兼容。

## 1. 开工审计：两条真缺口

| 路径 | 旧行为 | 后果 |
| --- | --- | --- |
| 复制 / 粘贴 | `copy_subtrees` + `paste_under` | 粘贴把概念挂成主选的子节点，制造 `parent/children` 树语义 |
| `Ctrl+D` | `duplicate_command` | 副本按兄弟节点插入，同样依赖父级 |
| `Ctrl+拖拽` | `commit_concept_move` 不看 `Moving.copy` | 概念图里按住 Ctrl 拖拽仍然只是移动，与思维导图不一致 |
| 选中反馈 | 渲染层只画 2px 环 | `docs/30` §5.3 要求的「选中 8% 强调底 / 悬停 4% 强调底」没有落地 |

概念图的边是命题，不是父子边；`docs/30` §5.2 明确禁止用 `parent_id` / `children`
表达层级。这四条路径都会把树语义漏回概念图。

## 2. 交付

### 2.1 数据与命令

新增 `Command::InsertConcepts { nodes }`（`mw-core/src/command.rs`）：

1. 仅概念图文档接受；
2. 每个节点必须 `parent_id = None`、`children = []`、`pos = Some(有限坐标)`；
3. id 必须唯一，关系线只保留指向本批或文档已有节点的目标；
4. 边界/概要属于树语义，插入时清空；
5. 卡片按 1:1 绑定校验，副本若沿用已占用卡片整条命令拒绝；
6. 一条命令插入全部概念，撤销一次全部收回。

### 2.2 交互路径

| 操作 | 现在的行为 |
| --- | --- |
| `Ctrl+C` / 菜单复制 | 只快照选中的概念本身，不取子树；只保留选中集内部的命题线 |
| `Ctrl+V` / 菜单粘贴 | 一律成为独立浮动概念，整体偏移 `(32, 44)`，不产生父子关系 |
| `Ctrl+D` | 选中概念原位复制，偏移 `(32, 44)`，整批一次撤销 |
| `Ctrl+拖拽` | 原件不动，副本落到拖拽位移处，整批一次撤销 |
| 跨模式粘贴 | 从思维导图复制到概念图时，所有节点扁平化为独立概念，无父子结构 |

副本会保留文本、样式、备注、标签、任务、图标、图片与内部命题（短语 / 颜色 / 箭头方向）；
卡片绑定不复制（副本解绑），避免落盘后 `validate` 判脏。

### 2.3 选中与悬停底色

`mw-render/src/instances.rs` 在画选中环之前先铺一层淡淡的强调色底：

- 悬停：`HOVER_FILL = 0.04`；
- 选中：`SELECT_FILL = 0.08`；
- 编辑态仍跳过，避免与编辑器重影。

这是共享渲染层，因此思维导图与概念图同时受益。底色在文字与节点内容之下，不改变可读性。

## 3. 测试

新增 5 条：

- `mw-core::document::tests::inserting_concepts_keeps_them_floating_and_undoes_in_one_step`
- `mw-core::document::tests::insert_concepts_is_concept_map_only_and_rejects_tree_payloads`
- `mw-core::document::tests::inserting_a_concept_does_not_steal_an_already_bound_card`
- `mw-shell::canvas::tests::concept_copy_preserves_propositions_but_never_parent_links`
- `mw-shell::canvas::tests::tree_clipboard_pastes_as_independent_concepts_without_parents`

全量（`--test-threads=1`，TEMP/TMP 重定向）：**968 passed / 0 failed / 1 ignored**（963 → 968）。
`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 与基线比新增 0。

## 4. 视觉漂移

26 个既有 shell 场景、1 张渲染画布基线 + 3 张 `shots/render/offscreen-zoom*.png` 因选中底色重拍；
新增 `cm-copy-paste` 一张基线：选中「用户痛点」→ `Ctrl+C` → `Ctrl+V` → 指针移开，画面里多出一颗
偏移 `(32,44)`、**没有父子线**的独立副本，且副本保持选中。轨迹三行：

```text
event clipboard Copy dirty=false
event concept-copy n=1 dx=32.0 dy=44.0
event clipboard Paste dirty=true
```

既有重拍差异全部落在被选中/悬停的节点盒内，最大通道差 ≤ 16：

| 场景 | 差像素 | 占比 | 最大通道差 | 包围盒 |
| --- | --- | --- | --- | --- |
| `cm-crosslink` | 7 763 | 0.337% | 16 | `(466,376)-(610,430)` |
| `mm-drag-child` | 11 592 | 0.663% | 14 | `(536,601)-(716,666)` |
| `selected` | 12 181 | 0.696% | 9 | `(546,407)-(738,470)` |

没有整页位移、文字重排或画布相机变化。

## 5. 机器门

| 门 | 结果 |
| --- | --- |
| stage-b | **PASS**，118 场 / 236 次比较，`--tol 0`，2026-10-30 13:36:57 |
| stage-cm | **PASS**，49 场 / 98 次比较，`--tol 0`，2026-10-30 13:48:59（新增 `cm-copy-paste` 后复跑） |

顺带修掉两条回归债：

1. X11–X16 / X20 的 manifest 仍指向已归档的 `docs/16-视觉重塑基准.md` → 改为
   `docs/archive/16-视觉重塑基准.md`，X11–X19 证据检查恢复 PASS。
2. 四个拖拽/折叠中间态脚本（`mm-drag-child/before/after`、`mm-toggle-edge`）在负载高时
   会抓到吸附前后两种态 → 末尾补 `sleep 160`，让 60ms 吸附判据与过渡先落定；
   四张基线连跑三次 `--tol 0` 零差异。

## 6. 未做 / 风险

- **未记 `已验证`**：都是离屏脚本与单测，真实鼠标连续拖拽、跨文档粘贴的手感未核。
- 卡片绑定不复制（副本解绑）是有意取舍；要复制卡片需要同时复制 `NoteCard` 与正文，
  属数据模型扩展，本轮不做。
- 跨模式粘贴会把树扁平化为概念，命题线只保留复制集合内部的那些；外部关系按设计丢弃。
- 选中底色是共享视觉变更，四主题 × 多 DPI 的人工色彩判读仍待门。
