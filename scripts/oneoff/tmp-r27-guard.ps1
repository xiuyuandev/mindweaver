$out = @("[golden] 离屏渲染 · 基线目录 shots/golden/x · 容差 0 · warmup 12 帧 · 超时 10s",
         "[golden] 输入脚本 shots/golden/inputs/x.txt · 25 帧")
$scriptFrames = 0; $warmupSeen = 0
foreach ($line in $out) {
  $t = [string]$line
  if ($t -match '输入脚本 (\S+) · (\d+) 帧') { $scriptFrames = [int]$Matches[2] }
  if ($t -match 'warmup (\d+) 帧') { $warmupSeen = [int]$Matches[1] }
}
if ($scriptFrames -gt 0) {
  if ($warmupSeen -lt $scriptFrames) { "FAIL script frames: warmup $warmupSeen < 脚本 $scriptFrames 帧" } else { "PASS $scriptFrames <= $warmupSeen" }
}