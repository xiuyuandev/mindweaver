#!/usr/bin/env bash
# 全量金样复核（绕开 `cargo golden`）。
#
# 为什么要绕：`cargo golden` 要抢 release 构建锁，跟另一个会话的构建撞上会
# 一直阻塞（本轮实测卡过四小时、零输出）。门脚本本质上只是「按场景设环境变量
# 再跑一次二进制」，直接驱动 `target/release/mw-shell.exe` 等价且不受锁影响。
#
# 场景表从 `scripts/x100-stage-b-gate.ps1` 抽取（见 `target/tmp/all-scenes.txt`）。
# 每行：名字|目录|ppp|warmup|环境变量
#
#   bash scripts/golden-sweep.sh            # 逐像素复核
#   bash scripts/golden-sweep.sh --update   # 重拍基线
set -u
cd "$(dirname "$0")/.." || exit 1
UPDATE=""
[ "${1:-}" = "--update" ] && UPDATE="--update"
BIN=./target/release/mw-shell.exe
LIST="${LIST:-target/tmp/all-scenes.txt}"
mkdir -p target/tmp
export TEMP="$PWD/target/tmp"
export TMP="$TEMP"

[ -f "$BIN" ] || { echo "缺 $BIN，先 cargo build --release -p mw-shell"; exit 2; }
[ -f "$LIST" ] || { echo "缺场景表 $LIST"; exit 2; }

ok=0
bad=0
while IFS='|' read -r name dir ppp warm env; do
  [ -z "$name" ] && continue
  app="$PWD/target/ci-appdata/sweep-$name"
  mkdir -p "$app" "$app-local"
  # 环境变量值里可能带空格（`MW_DRILL_DEMO=纯 Rust 原生`），所以用 `eval`
  # 让抽取阶段写好的引号生效；场景表是本仓库自己的，不接外部输入。
  #
  # 用绝对路径 `/usr/bin/env`：本机 `~/.local/bin/env` 有一个 shim 排在 PATH 前面，
  # 它会 exit 0 且不执行任何命令——2026-11-02 实测让 173 场全部假红（PASS=0 FAIL=173、
  # 错误详情为空）。别改回裸 `env`。
  out=$(eval "/usr/bin/env APPDATA=\"$app\" LOCALAPPDATA=\"$app-local\" $env \
        \"$BIN\" --golden --ppp \"$ppp\" --tol 0 --dir \"$dir\" \
        --warmup \"$warm\" $UPDATE" 2>&1)
  if echo "$out" | grep -q "无超容差差异"; then
    ok=$((ok + 1))
  elif echo "$out" | grep -q "基线已写入"; then
    ok=$((ok + 1))
  else
    bad=$((bad + 1))
    echo "DIFF $name :: $(echo "$out" | grep -E '像素超容差|失败|基线不存在' | head -1 | sed 's/.*\[golden\] //')"
  fi
done < "$LIST"
echo "PASS=$ok FAIL=$bad"
[ "$bad" -eq 0 ] || exit 1
