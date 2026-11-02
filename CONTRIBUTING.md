# 贡献指南

织思是 Windows 上的纯 Rust 原生思维导图 / 卡片笔记工具。欢迎提交问题、补测试、改文档与实现功能；动手前请先读 [HANDOFF.md](HANDOFF.md) 与 [docs/01 §5 非目标](docs/01-项目目标与适用范围.md)。

## 开发环境

- Windows 10 1809+ / Windows 11（x64）
- Rust stable（MSVC 工具链）
- Visual Studio Build Tools + Windows SDK（DirectWrite / WIC / COM）
- 不需要 Node.js、不需要 XMind；XMind 只作为可选视觉参考

```powershell
cargo run -p mw-shell --release
cargo test --workspace
powershell -File scripts\ci.ps1
```

## 提交前必须通过

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets`：**不新增警告**（基线指纹在 `scripts/clippy-baseline.txt`；测试数与告警数只看 `docs/STATUS.md` §1）
3. `cargo test --workspace`：全绿（串行见 `HANDOFF.md`，默认并行可能因 `note_store` 墙钟断言抖动）
4. 外观改动：跑三档 golden（1.0 / 1.5 / 2.0），确认差异是预期的；按 [HANDOFF.md](HANDOFF.md) 的量化和重拍纪律更新基线
5. 新功能：补单元测试；涉及布局/几何/序列化的，优先纯函数测试，不依赖窗口

CI 一条命令：

```powershell
powershell -File scripts\ci.ps1
# 无 GPU 机器：powershell -File scripts\ci.ps1 -SkipGolden
```

## 架构边界

```
mw-shell → mw-render / mw-core / mw-format / mw-io / mw-markdown
mw-render → mw-core
mw-format / mw-io → mw-core
mw-markdown 零内部依赖（连 mw-core 也不依赖）
mw-core 零 UI 依赖
```

- 文档修改只能走 `mw-core::Command`；不要直接改 `Document` 后绕过撤销。
- 画布、离屏 golden、PNG/SVG/PDF 导出共用 `Scene`；不要造第二套排版。
- 界面文案用中文；不引入 i18n / 英文界面。
- 不引入云同步、账号、遥测、付费/订阅、升级提示。

## 工作包施工协议（CER / SE / CM）

包清单与验收口径在 [docs/29](docs/29-内容体验重构工作包.md)（B2）与 [docs/31](docs/31-外壳体验与概念图工作包.md)（SE/CM）；
当前断点与下一步只看 [docs/STATUS.md](docs/STATUS.md) §5。每个包：

1. 先读包目标与验收口径；
2. 实现 + 测试，保证可编译；
3. 跑全量测试 / fmt / clippy / 必要 golden；
4. 在 `docs/STATUS.md` 更新该包状态与缺口；在 `shots/x100/<ID>/report.md` 写证据与**未做项**；
5. 没有达到验收就明确标为部分完成，不虚勾。

## 代码风格

- 注释说明**为什么**，不复述代码；中文标点，术语与现有代码一致。
- 用户可见字符串中文；错误信息给出可行动的原因。
- 不为一次性需求加抽象；但同一约束出现第三次时收敛到令牌/辅助函数。
- 性能敏感路径（布局、场景、绘制）避免每帧分配；大文档先看 `perf_scene` 与 `MW_STATS`。

## 真实输入与 golden

- 键盘/焦点问题优先用 `MW_TRACE=<file>` 记录轨迹，再配合截图；脚本放 `scripts/`（ASCII-only）。
- 改外观后先量化差异：位置、像素数、最大差；确认是环境（GPU/驱动/DPI）还是回归。
- 重拍三档基线：`cargo golden --ppp 1.0 --update`、`--ppp 1.5 --update`、`--ppp 2.0 --update`；重拍后连跑两次确认 0 差异，并在 `docs/x100/log/execution-log-2026-10.md` 记录漂移（`PLAN.md` 已是存根，不要再往里写逐轮记录）。

## 提交信息

建议格式：

```
feat(cards): 卡片反链解析
fix(canvas): 折叠后选择悬空
docs: 补 10 分钟用户手册
test(core): 1000 轮 undo/redo
```

正文写清：动机、行为变化、验证命令；外观改动附差异数字。

## 许可

仓库采用 MIT OR Apache-2.0 双许可（`LICENSE-MIT` / `LICENSE-APACHE`，说明见 `LICENSE`，均已就位）。提交即表示同意以该双许可发布你的贡献。
