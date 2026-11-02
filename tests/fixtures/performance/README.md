# 性能测试语料

本目录保存大文档性能测试用的 `.zmind` 文件。它们原本散落在 `target/`，2026-10-24 清理时迁移到此。

## 文件

| 文件 | 说明 |
| --- | --- |
| `big-20k.zmind` | 20,000 节点大文档 |
| `big-20k.autosave.zmind` | 对应自动保存点 |
| `perf-20k.zmind` | X100 性能 fixture（20,000 节点） |

## 使用

```powershell
# 启动并打开大文档
cargo run -p mw-shell --release -- tests/fixtures/performance/big-20k.zmind

# 内存稳态测试
powershell -File scripts/m5-memory.ps1 -Exe target/release/mw-shell.exe `
  -Doc tests/fixtures/performance/big-20k.zmind -Minutes 5 -Tag 20k-idle
```

> 这些语料只用于本地性能/回归测试，不提交 golden 基线。
