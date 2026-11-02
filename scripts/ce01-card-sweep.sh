#!/usr/bin/env bash
# CE-01 证据：把全部卡片场景按 stage-b 门里的配置跑一遍，只比不改。
# 每行：名字|ppp|warmup|环境变量|输入脚本
set -u
cd "$(dirname "$0")/.." || exit 1
# 加 `--update` 就是重拍基线；不加就是逐像素复核。
UPDATE=""
[ "${1:-}" = "--update" ] && UPDATE="--update"
BIN=./target/release/mw-shell.exe
# 与门脚本同款：本机受限令牌写不了系统 %TEMP%，笔记自动保存会失败并弹红条，
# 那不是代码问题，是没有重定向（见 HANDOFF §3.1）。
mkdir -p target/tmp
export TEMP="$PWD/target/tmp"
export TMP="$TEMP"
SCENES="
card-default|1.5|3|MW_CARD_DEMO=1|
card-empty|1.5|3|MW_CARD_EMPTY=1|
card-long|1.5|3|MW_CARD_LONG=1|
card-narrow|1.5|3|MW_CARD_LONG=1 MW_WINDOW_SIZE=900x700|
card-tags|1.5|3|MW_CARD_TAGS=1|
card-slash|1.5|20|MW_CARD_DEMO=1 MW_CARD_SLASH=1|
card-slash-filter|1.5|20|MW_CARD_DEMO=1 MW_CARD_SLASH=1|shots/golden/inputs/card-slash-filter.txt
card-slash-scroll|1.5|20|MW_CARD_DEMO=1 MW_CARD_SLASH=1|shots/golden/inputs/card-slash-scroll.txt
card-blocks|1.0|3|MW_CARD_BLOCKS=1 MW_WINDOW_SIZE=1200x1000|
card-reveal|1.0|12|MW_CARD_REVEAL=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-reveal.txt
card-reveal-block|1.0|12|MW_CARD_REVEAL=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-reveal-block.txt
card-reveal-link|1.0|12|MW_CARD_REVEAL_LINK=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-reveal-link.txt
card-enter-continue|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-enter-continue.txt
card-enter-exit|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-enter-exit.txt
card-blank-caret|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-blank-caret.txt
card-instant|1.0|40|MW_CARD_INSTANT=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-instant.txt
card-instant-undo|1.0|40|MW_CARD_INSTANT=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-instant-undo.txt
card-task-click|1.0|20|MW_CARD_TASK=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-task-click.txt
card-paste-md|1.0|20|MW_CARD_TASK=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-paste-md.txt
card-saving|1.0|20|MW_CARD_STATE=saving MW_WINDOW_SIZE=1200x1000|
card-conflict|1.0|20|MW_CARD_STATE=conflict MW_WINDOW_SIZE=1200x1000|
card-readonly|1.0|20|MW_CARD_STATE=readonly MW_WINDOW_SIZE=1200x1000|
card-autosave|1.0|20|MW_CARD_TASK=1 MW_CARD_AUTOSAVE=50 MW_CARD_NOW_MS=1792800000000 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-autosave.txt
card-editing|1.0|20|MW_CARD_TASK=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-editing.txt
card-20k|1.0|12|MW_CARD_20K=1 MW_WINDOW_SIZE=1200x1000|
card-code|1.0|20|MW_CARD_CODE=1 MW_WINDOW_SIZE=1200x1000|
card-selection|1.0|20|MW_CARD_DEMO=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-selection.txt
card-ime-preedit|1.0|24|MW_CARD_IME=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-ime-preedit.txt
card-ime-commit|1.0|24|MW_CARD_IME=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-ime-commit.txt
card-paste-list|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-paste-list.txt
card-paste-none|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-paste-none.txt
card-paste-undo|1.0|20|MW_CARD_ENTER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-paste-undo.txt
card-focus|1.5|20|MW_CARD_DEMO=1 MW_CARD_FOCUS=1|shots/golden/inputs/card-focus.txt
card-typewriter|1.5|20|MW_CARD_TALL=1 MW_CARD_TYPEWRITER=1 MW_WINDOW_SIZE=1200x1000|shots/golden/inputs/card-typewriter.txt
"
DRIFT=0
echo "$SCENES" | while IFS='|' read -r name ppp warm env input; do
  [ -z "$name" ] && continue
  if [ -n "$input" ]; then
    export MW_GOLDEN_INPUT="$input"
  else
    unset MW_GOLDEN_INPUT
  fi
  # 与门的 `Run-GoldenScene` 同款：每个场景一套 APPDATA/LOCALAPPDATA。
  # 不隔离的话程序会去读写本机真实笔记库，受限令牌下写盘失败 → 弹红色
  # 「自动保存失败」提示条，整条基线就全红了（那不是代码问题）。
  app="$PWD/target/ci-appdata/ce01-$name"
  mkdir -p "$app" "$app-local"
  out=$(env APPDATA="$app" LOCALAPPDATA="$app-local" $env $BIN --golden --ppp "$ppp" --tol 0 \
        --dir "shots/golden/$name" --warmup "$warm" $UPDATE 2>&1)
  if echo "$out" | grep -q "无超容差差异"; then
    echo "OK   $name"
  else
    echo "DIFF $name :: $(echo "$out" | tail -3 | tr '\n' ' ')"
  fi
done
echo "--- 完 ---"
