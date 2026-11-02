#!/usr/bin/env bash
# X21b 漂移取证脚本（只在 shots/x100/X21b/ 下写盘，不改基线、不改门脚本）。
#
# 用法：
#   bash shots/x100/X21b/run-golden.sh before   # 改码前：核基线绿 + 记旧哈希 + 旧图留档
#   bash shots/x100/X21b/run-golden.sh after    # 改码后：量漂移（同一命令）
#   bash shots/x100/X21b/run-golden.sh update   # 只重拍代表场景（3–5 场留证）
#
# 环境与 `scripts/x100-stage-b-gate.ps1` 的 Run-GoldenScene 同构：
#   APPDATA / LOCALAPPDATA 每场隔离，TEMP/TMP 指仓库内，MW_* 旋钮照门里的表。
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 2
export TEMP="$ROOT/target/tmp"
export TMP="$ROOT/target/tmp"
mkdir -p "$TEMP" shots/x100/X21b/out

EXE="$ROOT/target/release/mw-shell.exe"
MODE="${1:-before}"
LOG="shots/x100/X21b/out/$MODE.log"
: > "$LOG"

# 场景表：名字 | 基线目录 | ppp | warmup | 额外环境（分号分隔 K=V）
SCENES=(
  "theme-ink|shots/golden/theme-ink|1.5|3|MW_THEME=ink"
  "theme-sand|shots/golden/theme-sand|1.5|3|MW_THEME=sand"
  "mm-collapse|shots/golden/mm-collapse|1.0|30|MW_GOLDEN_INPUT=shots/golden/inputs/mm-collapse.txt"
  "mm-minimap|shots/golden/mm-minimap|1.0|8|MW_MINIMAP=1;MW_NOTES=none;MW_WINDOW_SIZE=1280x800"
  "cm-default|shots/golden/cm-default|1.0|6|MW_CONCEPT_DEMO=1;MW_WINDOW_SIZE=1280x800"
)

for row in "${SCENES[@]}"; do
  IFS='|' read -r name dir ppp warm envs <<< "$row"
  app="$ROOT/target/ci-appdata/x21b-$name"
  applocal="$app-local"
  mkdir -p "$app" "$applocal"
  extra=()
  if [ "$MODE" = "update" ]; then extra+=(--update); fi
  echo "===== $name ($dir, ppp=$ppp, warmup=$warm, $envs)" | tee -a "$LOG"
  env APPDATA="$app" LOCALAPPDATA="$applocal" \
      $(for kv in ${envs//;/ }; do printf '%s ' "$kv"; done) \
      "$EXE" --golden --ppp "$ppp" --tol 0 --warmup "$warm" --dir "$dir" "${extra[@]}" 2>&1 | tee -a "$LOG"
  echo "exit=$?" | tee -a "$LOG"
done

# 旧/新哈希对照（基线文件本身）
echo "===== sha256 (baseline files)" | tee -a "$LOG"
sha256sum shots/golden/{theme-ink,theme-sand,mm-collapse,mm-minimap,cm-default}/shell-*.png 2>/dev/null | tee -a "$LOG"
