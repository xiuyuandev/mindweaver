param(
    [switch]$SkipFullTests,
    [switch]$SkipGolden,
    [switch]$SkipClippy,
    # 重拍基线：每个场景照同一份环境表跑一遍 --update，第二遍就是对刚写下的
    # 基线做逐像素复核。只有「外观有意改动」时才用，日志里会写明。
    [switch]$Update
)

# X100 Stage B 机器门（X11–X20）。
# 覆盖：证据包完整性、fmt、mw-shell clippy 新增 0、全量串行测试、golden 全矩阵双跑
# （场次与比较数以 `docs/STATUS.md` §1 为准；这里不写死数字，免得又过期）。
# 用法：pwsh -File scripts/x100-stage-b-gate.ps1
# 退出码：0 = 机器门通过（仍需 X20 人工验收）；非 0 = 第一处失败。
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot

# 本机用户 Temp（%LOCALAPPDATA%\Temp）对测试进程**偶发拒写**（os error 5 拒绝访问），
# 会让所有写盘用例假失败（mw-shell 56 条 / mw-format 8 条）。测试与夹具一律落在仓库内，
# 这里显式把 TEMP/TMP 指过去——CI 的结论不该受本机 Temp 权限影响。
$scratch = Join-Path $root "target/tmp/ci-temp"
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$env:TEMP = $scratch
$env:TMP = $scratch
Set-Location $root
$logDir = Join-Path $root 'target/x100'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
# 测试会往 $env:TEMP 写临时目录（`.zmind` 往返用例）。在受限令牌下系统 Temp 可能不可写
# （2026-10-28 实测 `os error 5 拒绝访问`，mw-format 8 个用例假失败），统一把 TEMP/TMP 指到仓库内。
# 这两行与 `ci.ps1` / `x100-stage-cm-gate.ps1` 保持一致；漏掉就是「同一个环境问题换一个脚本再犯一次」。
$repoTemp = Join-Path $root 'target\tmp'
New-Item -ItemType Directory -Force -Path $repoTemp | Out-Null
$env:TEMP = (Resolve-Path $repoTemp).Path
$env:TMP = $env:TEMP
$log = New-Object System.Collections.Generic.List[string]
$failures = New-Object System.Collections.Generic.List[string]

function Say([string]$text) {
    Write-Host $text
    $log.Add($text)
}

function Run-Step([string]$name, [scriptblock]$body) {
    Say ""
    Say "==> $name"
    $global:LASTEXITCODE = 0
    try {
        $output = & $body 2>&1
        $code = $LASTEXITCODE
        $ok = $?
    } catch {
        $output = @($_)
        $code = 1
        $ok = $false
    }
    foreach ($line in $output) { Say ([string]$line) }
    if (-not $ok -or $code -ne 0) {
        $failures.Add("$name (exit $code)")
        Say "FAIL $name"
        return $false
    }
    Say "PASS $name"
    return $true
}

function Get-TestCounts($output) {
    $passed = 0
    $failed = 0
    foreach ($line in $output) {
        if ([string]$line -match '^test result: ok\.\s+(\d+) passed;\s+(\d+) failed') {
            $passed += [int]$Matches[1]
            $failed += [int]$Matches[2]
        }
    }
    return @{ Passed = $passed; Failed = $failed }
}

# 173 场（B2-C 笔记库 +18 场，见下方 library 段；CER-12 补拍 +outline-wrap 1.5/2.0）。
# shell、library、theme 各跨 1.0/1.5/2.0 三档。场数与比较数以 docs/STATUS.md §1 为准，这行只做规模提示。
$scenes = @(
    @{ Name = 'base-ppp1.0'; Dir = 'shots/golden'; Ppp = '1.0'; Warmup = 3; Env = @{} },
    @{ Name = 'base-ppp1.5'; Dir = 'shots/golden'; Ppp = '1.5'; Warmup = 3; Env = @{} },
    @{ Name = 'base-ppp2.0'; Dir = 'shots/golden'; Ppp = '2.0'; Warmup = 3; Env = @{} },
    @{ Name = 'library-ppp1.0'; Dir = 'shots/golden/library'; Ppp = '1.0'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = '1' } },
    @{ Name = 'library-ppp1.5'; Dir = 'shots/golden/library'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = '1' } },
    @{ Name = 'library-ppp2.0'; Dir = 'shots/golden/library'; Ppp = '2.0'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = '1' } },
    # CER-22–30（B2-C 笔记库）：`library` 三档 ppp 是「固定物理尺寸变 DPI」的自适应矩阵
    # （物理 = MW_WINDOW_SIZE×1.5，再被显示器 1920x1200 夹；css = 物理/ppp = 1620/1080/810
    # → 三/两/单栏各一张），其余 18 场按 `docs/28` §7.10 的验收场景逐条铺开。
    # 全部只靠 MW_LIBRARY_DEMO / MW_WINDOW_SIZE / MW_THEME 三个旋钮驱动，不加输入脚本；
    # 夹具目录按场景分开（MW_DEMO_LIB 未设时是 target/tmp/mw-library-demo-<scene>），
    # 否则上一场的残留会让 `empty` 悄悄变成 20 张卡。
    @{ Name = 'library-three-pane'; Dir = 'shots/golden/library-three-pane'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '1280x800' } },
    # 四主题的内容令牌（snow 即上面三档 ppp + three-pane；另三套各一张列表态、一张阅读态）。
    @{ Name = 'library-ink'; Dir = 'shots/golden/library-ink'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'ink' } },
    @{ Name = 'library-mist'; Dir = 'shots/golden/library-mist'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'mist' } },
    @{ Name = 'library-sand'; Dir = 'shots/golden/library-sand'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'sand' } },
    # 两栏档的预览是整页阅读面（不是 300px 窄栏）：`library-preview` 拍阅读面本身，
    # `library-two-pane` 拍「无选中时预览不占位」——两者环境不同（demo 与选中态）。
    @{ Name = 'library-preview'; Dir = 'shots/golden/library-preview'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'preview'; MW_WINDOW_SIZE = '1080x720' } },
    @{ Name = 'library-two-pane'; Dir = 'shots/golden/library-two-pane'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '1080x720' } },
    # 单栏两档：合同 §7.10 列 800x600 / 640x460，这里拍 800x720（与门内其余场同高）与 640x460。
    @{ Name = 'library-one-pane'; Dir = 'shots/golden/library-one-pane'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '800x720' } },
    @{ Name = 'library-one-pane-narrow'; Dir = 'shots/golden/library-one-pane-narrow'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'default'; MW_WINDOW_SIZE = '640x460' } },
    # 三态互斥：库里一张卡都没有 / 有卡但零命中 / 有卡在但读不到（骨架）。
    @{ Name = 'library-empty'; Dir = 'shots/golden/library-empty'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'empty'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-no-results'; Dir = 'shots/golden/library-no-results'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'no-results'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-loading'; Dir = 'shots/golden/library-loading'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'loading'; MW_WINDOW_SIZE = '1280x800' } },
    # 10k 虚拟化首屏 + 超长标题/摘要省略；选择模式与批量条；标签 AND 过滤（含停靠预览 = snow 阅读档）。
    @{ Name = 'library-long'; Dir = 'shots/golden/library-long'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'long'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-select'; Dir = 'shots/golden/library-select'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'select'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-tag-filter'; Dir = 'shots/golden/library-tag-filter'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'tag-filter'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-archive'; Dir = 'shots/golden/library-archive'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'archive'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'library-preview-ink'; Dir = 'shots/golden/library-preview-ink'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'preview'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'ink' } },
    @{ Name = 'library-preview-mist'; Dir = 'shots/golden/library-preview-mist'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'preview'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'mist' } },
    @{ Name = 'library-preview-sand'; Dir = 'shots/golden/library-preview-sand'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_LIBRARY_DEMO = 'preview'; MW_WINDOW_SIZE = '1280x800'; MW_THEME = 'sand' } },
    @{ Name = 'theme-ink'; Dir = 'shots/golden/theme-ink'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_THEME = 'ink' } },
    @{ Name = 'theme-mist'; Dir = 'shots/golden/theme-mist'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_THEME = 'mist' } },
    @{ Name = 'theme-sand'; Dir = 'shots/golden/theme-sand'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_THEME = 'sand' } },
    @{ Name = 'outline'; Dir = 'shots/golden/outline'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1' } },
    @{ Name = 'outline-focus'; Dir = 'shots/golden/outline-focus'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_FOCUS = '1' } },
    # CER-13 卡片页骨架：这三张此前**没有进任何门**（只有 card-slash 在门里），基线过期也发现不了。
    @{ Name = 'card-default'; Dir = 'shots/golden/card-default'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_CARD_DEMO = '1' } },
    @{ Name = 'card-empty'; Dir = 'shots/golden/card-empty'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_CARD_EMPTY = '1' } },
    # 版心标尺：长中文段落在 720 处换行（实测 `车`左 180.0、换行右 890.7–892.7、宽 710.7–712.7 = 720 减字形右衬）。
    @{ Name = 'card-long'; Dir = 'shots/golden/card-long'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_CARD_LONG = '1' } },
    # 窄窗退 640：窗口 900 → 左 130.0（=(900-640)/2）、换行右 764.7、宽 634.7。
    @{ Name = 'card-narrow'; Dir = 'shots/golden/card-narrow'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_CARD_LONG = '1'; MW_WINDOW_SIZE = '900x700' } },
    # CER-14：标签 / 元信息 / 来源 / 反链 / 断链——一张卡片的页头与页尾全在这一张里。
    @{ Name = 'card-tags'; Dir = 'shots/golden/card-tags'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_CARD_TAGS = '1' } },
    # CER-18：斜杠菜单。warmup 20 —— 菜单要等焦点与光标落定才出现（warmup 3 时它还没开，会拍出一张没有浮层的假基线）。
@{ Name = 'card-slash'; Dir = 'shots/golden/card-slash'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_DEMO = '1'; MW_CARD_SLASH = '1' } },
# 过滤：菜单里再敲 `h2`，只剩「标题 2」。
@{ Name = 'card-slash-filter'; Dir = 'shots/golden/card-slash-filter'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_DEMO = '1'; MW_CARD_SLASH = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-slash-filter.txt' } },
# 滚动：10 条、最多 8 行，连按 8 次 ↓ 把高亮推到底，菜单滚动而高亮整行可见。
@{ Name = 'card-slash-scroll'; Dir = 'shots/golden/card-slash-scroll'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_DEMO = '1'; MW_CARD_SLASH = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-slash-scroll.txt' } },
    # CER-15 块级排版：H2/H3/H4、段落、有序/无序/任务列表、引用、行内码、围栏代码块、分隔线
    # 与「CRLF 正文」一次性钉住。窗口要够高（正文到 872px），否则代码块落在折叠线以下。
    @{ Name = 'card-blocks'; Dir = 'shots/golden/card-blocks'; Ppp = '1.0'; Warmup = 3; Env = @{ MW_CARD_BLOCKS = '1'; MW_WINDOW_SIZE = '1200x1000' } },
    # CER-16 片段级揭示：光标停在 `**粗体**` 里，只露这一段的 `**`（同一行的反引号不露）。
    @{ Name = 'card-reveal'; Dir = 'shots/golden/card-reveal'; Ppp = '1.0'; Warmup = 12; Env = @{ MW_CARD_REVEAL = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-reveal.txt' } },
    # CER-16 块级标记揭示：光标停在 `## 小标题` 行 → 只有 `##` 露出，段落/列表行的标记仍是成稿态。
    @{ Name = 'card-reveal-block'; Dir = 'shots/golden/card-reveal-block'; Ppp = '1.0'; Warmup = 12; Env = @{ MW_CARD_REVEAL = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-reveal-block.txt' } },
    # CER-16 链接片段：光标进 `[织思](url)` → 方括号与 URL 露出；同一行的 `[[双链]]` 仍收起。
    @{ Name = 'card-reveal-link'; Dir = 'shots/golden/card-reveal-link'; Ppp = '1.0'; Warmup = 12; Env = @{ MW_CARD_REVEAL_LINK = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-reveal-link.txt' } },
    # CER-16 后半：光标在第二项行尾按回车 → 新行自己带上 “- ”，接着打字仍是列表项（首簇是 5px 短横）。
    @{ Name = 'card-enter-continue'; Dir = 'shots/golden/card-enter-continue'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-enter-continue.txt' } },
    # CER-16 后半：续写出的空项再按一次回车 → 前缀退掉，这一行变回空段落（首簇直接是字，没有短横）。
    @{ Name = 'card-enter-exit'; Dir = 'shots/golden/card-enter-exit'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-enter-exit.txt' } },
    # CE-01（卡片编辑体验）：空列表项回车退成空段落，光标停在那一行。
    # 改前空行的行高被压成 0，光标矩形 (0,28)-(0,28)——屏幕上看不见。
    @{ Name = 'card-blank-caret'; Dir = 'shots/golden/card-blank-caret'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-blank-caret.txt' } },
    # CE-01：超长正文下窗口右缘的细滚动条（悬停淡入）。
    @{ Name = 'card-scroll'; Dir = 'shots/golden/card-scroll'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_TALL = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-scroll.txt' } },
    # CER-16 后半：即时转换。第二段行尾回车后逐格敲「- 」「1. 」「## 」，每敲完那格空格就成块；
    # `card-instant-undo` 是同一动作 + 一次 Ctrl+Z，与不进基线的 card-instant-enter（只回车）
    # **逐字节相同**，证明「一次撤销整枚标记」。warmup 40 要盖住 30+ 帧输入脚本。
    @{ Name = 'card-instant'; Dir = 'shots/golden/card-instant'; Ppp = '1.0'; Warmup = 40; Env = @{ MW_CARD_INSTANT = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-instant.txt' } },
    @{ Name = 'card-instant-undo'; Dir = 'shots/golden/card-instant-undo'; Ppp = '1.0'; Warmup = 40; Env = @{ MW_CARD_INSTANT = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-instant-undo.txt' } },
    # CER-17：点第一条待办的勾选框（16×16，中心 547,272 @ ppp 1.0）→ 正文从 `- [ ] 待办一`
    # 翻成 `- [x] 待办一`，方框由 1.2px 描边变强调色实心；同帧不动其它字符（trace `card_task_toggle at=8 checked=false line_end=14`）。
    @{ Name = 'card-task-click'; Dir = 'shots/golden/card-task-click'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_TASK = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-task-click.txt' } },
    # CER-17：结构化粘贴。光标在待办行尾粘一段自带 `## `- `- ` 的成稿 → 原行不动、
    # 整段原样落成自己的块（不补 `- ` 前缀）。对照组 card-paste-plain 不进基线（粘纯文本按当前块补 `- [ ] `）。
    @{ Name = 'card-paste-md'; Dir = 'shots/golden/card-paste-md'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_TASK = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-paste-md.txt' } },
    # CER-20：保存管道的三种顶栏状态。同一张卡、同样没有输入，差别只在状态字与那个 6px 点。
    @{ Name = 'card-saving'; Dir = 'shots/golden/card-saving'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_STATE = 'saving'; MW_WINDOW_SIZE = '1200x1000' } },
    @{ Name = 'card-conflict'; Dir = 'shots/golden/card-conflict'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_STATE = 'conflict'; MW_WINDOW_SIZE = '1200x1000' } },
    @{ Name = 'card-readonly'; Dir = 'shots/golden/card-readonly'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_STATE = 'readonly'; MW_WINDOW_SIZE = '1200x1000' } },
    # CER-20：自动保存走完一轮（阈值压到 50ms + 脚本 sleep 200ms → 离边界很远，判定稳定）。
    @{ Name = 'card-autosave'; Dir = 'shots/golden/card-autosave'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_TASK = '1'; MW_CARD_AUTOSAVE = '50'; MW_CARD_NOW_MS = '1792800000000'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-autosave.txt' } },
    # 对照：同样的输入，但不自动保存 → 顶栏停在「编辑中 · 稍后自动保存」。
    @{ Name = 'card-editing'; Dir = 'shots/golden/card-editing'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_TASK = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-editing.txt' } },
    # CER-20 长文：20k 中文字符的一页（只拍首屏；打开/输入/IME 的毫秒数在 `target/x100/cer20/` 的探针里）。
    @{ Name = 'card-20k'; Dir = 'shots/golden/card-20k'; Ppp = '1.0'; Warmup = 12; Env = @{ MW_CARD_20K = '1'; MW_WINDOW_SIZE = '1200x1000' } },
    # CER-21：`docs/28` §6.11 里补齐的两类场景。`card-code` 同页放代码块 + 行内码 + 链接 + 图片语法；
    # `card-selection` 点进正文 → Ctrl+A 全选 → Shift+← 收三格（选区跨块、锚点留在文首）。
    @{ Name = 'card-code'; Dir = 'shots/golden/card-code'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_CODE = '1'; MW_WINDOW_SIZE = '1200x1000' } },
    @{ Name = 'card-selection'; Dir = 'shots/golden/card-selection'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_DEMO = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-selection.txt' } },
    # CER-16 / CER-17：IME 组合中——组合串原地长在光标处并带下划线；两段的 y 与成稿态逐像素相同（组合不参与排版）。
    @{ Name = 'card-ime-preedit'; Dir = 'shots/golden/card-ime-preedit'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CARD_IME = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-ime-preedit.txt' } },
    # 同一脚本再确认：下划线消失、字号字距不动，前后行距一个像素都没变（与 preedit 图只差 1792 px）。
    @{ Name = 'card-ime-commit'; Dir = 'shots/golden/card-ime-commit'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CARD_IME = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-ime-commit.txt' } },
    # CER-17：多行纯文本粘进列表项 —— 后一行也带 “- ”（首簇 543-547 是短横）；段落里粘则原样两行（对照 `paste-plain` 不进基线）。
    @{ Name = 'card-paste-list'; Dir = 'shots/golden/card-paste-list'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-paste-list.txt' } },
    # CER-17 一次撤销：`card-paste-none` 是「什么都没粘」的参照，`card-paste-undo` 是「粘两行再按一次 Ctrl+Z」。
    # 两张基线**逐字节相同**（C32274AB09AD）——这条不变量本身就是判据：撤销必须等于没发生过。
    @{ Name = 'card-paste-none'; Dir = 'shots/golden/card-paste-none'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-paste-none.txt' } },
    @{ Name = 'card-paste-undo'; Dir = 'shots/golden/card-paste-undo'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CARD_ENTER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-paste-undo.txt' } },
    # CER-19：专注模式——光标所在标题块 100%，其余块文字 alpha 压到 35%（不加遮罩，背景像素不变）。
    @{ Name = 'card-focus'; Dir = 'shots/golden/card-focus'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_DEMO = '1'; MW_CARD_FOCUS = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-focus.txt' } },
    # CER-19：打字机——长正文里 Ctrl+End 后连按两次回车，光标被锚到可视区 42%。
    @{ Name = 'card-typewriter'; Dir = 'shots/golden/card-typewriter'; Ppp = '1.5'; Warmup = 20; Env = @{ MW_CARD_TALL = '1'; MW_CARD_TYPEWRITER = '1'; MW_WINDOW_SIZE = '1200x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/card-typewriter.txt' } },
    @{ Name = 'outline-compact'; Dir = 'shots/golden/outline-compact'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_DENSITY = 'compact' } },
    # CER-09：握把悬停 120ms 才出现；hover-select 基线沿用它原有的 1200×720 窗口，
    # 避免把窗口宽度差异混进这次漂移（实测重拍前后逐字节相同）。
    @{ Name = 'outline-hover-select'; Dir = 'shots/golden/outline-hover-select'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_WINDOW_SIZE = '1200x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-hover-select.txt' } },
    # M25：行尾 ⋯ 与行右键菜单（docs/28 §5.2「更多 16px ⋯ 距列右 8，悬停/选中」）。
    # more 悬停带说明行的「中文排版」，同时钉住备注圆点左移后与 ⋯ 的间隙；
    # row-menu 右键锚在行内空白处，菜单向右展开不压文本；row-menu-add 走 ↓/Enter
    # 建子主题并进入编辑态（编辑光标用 MW_OUTLINE_EDIT 固定常亮）。
    @{ Name = 'outline-more'; Dir = 'shots/golden/outline-more'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_WINDOW_SIZE = '1200x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-more.txt' } },
    @{ Name = 'outline-row-menu'; Dir = 'shots/golden/outline-row-menu'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_WINDOW_SIZE = '1200x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-row-menu.txt' } },
    @{ Name = 'outline-row-menu-add'; Dir = 'shots/golden/outline-row-menu-add'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_EDIT = '1'; MW_WINDOW_SIZE = '1200x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-row-menu-add.txt' } },
    # CER-10：筛选条——文本「排版」+ 标签「验收」AND，命中 12% 底、祖先 48%、
    # 条件胶囊与计数；`outline-search-labels` 单独锁标签弹层。
    @{ Name = 'outline-search'; Dir = 'shots/golden/outline-search'; Ppp = '1.5'; Warmup = 6; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_SEARCH = '排版'; MW_OUTLINE_FILTER_LABEL = '验收' } },
    @{ Name = 'outline-search-labels'; Dir = 'shots/golden/outline-search-labels'; Ppp = '1.5'; Warmup = 6; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_LABEL_POPUP = '1' } },
    # CER-11：20k 行虚拟化——顶部与滚到「分支 0150」（全表第 751 行）两帧；
    # 只排视口 ±10 行，离屏行只留空白占位。
    @{ Name = 'outline-20k'; Dir = 'shots/golden/outline-20k'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_20K = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # warmup 10 在门串行负载下不够：MW_OUTLINE_SCROLL_TO 的滚动落定没跑完，第 1 遍会抓到
    # 半途的画面（2026-10-28 实测：第 1 遍 0.496%、第 2 遍零差异；单独复现该命令 10/16 都逐位相同）。
      @{ Name = 'outline-20k-deep'; Dir = 'shots/golden/outline-20k-deep'; Ppp = '1.0'; Warmup = 16; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_20K = '1'; MW_OUTLINE_SCROLL_TO = '分支 0150'; MW_WINDOW_SIZE = '1280x800' } },
    # CER-09：拖拽预览——源行 40%、目标行 6% 底；illegal 是同一手势落进后代，红 1px + NoDrop。
    @{ Name = 'outline-drag'; Dir = 'shots/golden/outline-drag'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-drag.txt' } },
    @{ Name = 'outline-drag-illegal'; Dir = 'shots/golden/outline-drag-illegal'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_OUTLINE = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-drag-illegal.txt' } },
    # CER-12 补拍：docs/28 §5.2 非编辑态长文换行（同夹具 1.5 / 2.0 两档）。
    @{ Name = 'outline-wrap'; Dir = 'shots/golden/outline-wrap'; Ppp = '1.5'; Warmup = 6; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_WRAP = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'outline-wrap-2x'; Dir = 'shots/golden/outline-wrap'; Ppp = '2.0'; Warmup = 6; Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_WRAP = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'selected'; Dir = 'shots/golden/selected'; Ppp = '1.5'; Warmup = 3; Env = @{ MW_SELECT_DEMO = '1' } },
    @{ Name = 'menu'; Dir = 'shots/golden/menu'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_CONTEXT_DEMO = '1' } },
    @{ Name = 'menu-narrow'; Dir = 'shots/golden/menu-narrow'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_CONTEXT_DEMO = '1'; MW_WINDOW_SIZE = '640x460' } },
    @{ Name = 'menu-canvas-click'; Dir = 'shots/golden/menu-canvas-click'; Ppp = '1.5'; Warmup = 24; Env = @{ MW_WINDOW_SIZE = '1080x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/menu-canvas-click.txt' } },
    @{ Name = 'dialog'; Dir = 'shots/golden/dialog'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_DIALOG_DEMO = '1' } },
    # 打磨轮 50 / M27：快捷键面板的两种状态。验收旋钮只写内存、不落盘。
    @{ Name = 'shortcuts-default'; Dir = 'shots/golden/shortcuts-default'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_SHORTCUTS_DEMO = '1' } },
    @{ Name = 'shortcuts-disabled'; Dir = 'shots/golden/shortcuts-disabled'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_SHORTCUTS_DEMO = 'save,undo,redo' } },
    # 打磨轮 51 / M28：设置中心「编辑」组（撤销栈上限 100–2000，默认 500）。
    @{ Name = 'settings'; Dir = 'shots/golden/settings'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_SETTINGS_DEMO = '1' } },
    # 打磨轮 59（M28b 撤销上限调低的提示）：先点根 + 110 次 Enter 把撤销深度堆到 127，
    #   第 240 帧自动开设置（MW_SETTINGS_AFTER），草稿上限 100 → 滑轨下方 28px 警告行
    #   「调低会立即丢弃最旧的 27 步撤销记录」。笔记库路径固定，基线不随 APPDATA 漂。
    @{ Name = 'settings-undo-warn'; Dir = 'shots/golden/settings-undo-warn'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_SETTINGS_DRAFT_UNDO = '100'; MW_SETTINGS_AFTER = '240'; MW_SETTINGS_NOTES_DIR = 'C:/织思笔记'; MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/settings-undo-warn.txt' } },
    @{ Name = 'start'; Dir = 'shots/golden/start'; Ppp = '1.0'; Warmup = 3; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_WINDOW_SIZE = '1440x1000' } },
    # 起始页有「纵向余量摊分」的落定过程（SE-01）：第一帧拿不到内容高，
    # 第二帧才摊分，窄窗还要等滚动条出现改变换行。warmup 给到 8 才稳定。
    @{ Name = 'start-narrow'; Dir = 'shots/golden/start-narrow'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_WINDOW_SIZE = '720x620' } },
    @{ Name = 'start-scroll'; Dir = 'shots/golden/start-scroll'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/start-scroll.txt' } },
    @{ Name = 'start-tab-focus'; Dir = 'shots/golden/start-tab-focus'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/start-tab-focus.txt' } },
    # SE-02（打磨轮 20）：起始页的状态与四主题。
    #   `start-empty`：没有最近文件时的两行空状态（宽窗，不与 start-narrow 的窄断点混在一起）。
    #   `start-skeleton`：最近文件的 3 行加载骨架。真实运行是异步读盘，金样里用
    #     `MW_RECENTS_PENDING_FRAMES` 假装「还没读完」——取帧时必须在预算之内，否则拍到的是真列表。
    #   `start-concept`：指针压在「新建概念图」卡上（卡片矩形见轨迹 `quick rect concept`），锁 4% hover 底。
    #   `start-ink/-mist/-sand`：同一页在另外三套主题下（默认主题就是 `start` 那张）。
    @{ Name = 'start-empty'; Dir = 'shots/golden/start-empty'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_WINDOW_SIZE = '1440x1000' } },
    @{ Name = 'start-skeleton'; Dir = 'shots/golden/start-skeleton'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_RECENTS_PENDING_FRAMES = '20'; MW_WINDOW_SIZE = '1440x1000' } },
    @{ Name = 'start-concept'; Dir = 'shots/golden/start-concept'; Ppp = '1.0'; Warmup = 16; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_WINDOW_SIZE = '1440x1000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/start-concept.txt' } },
    @{ Name = 'start-ink'; Dir = 'shots/golden/start-ink'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_THEME = 'ink'; MW_WINDOW_SIZE = '1440x1000' } },
    @{ Name = 'start-mist'; Dir = 'shots/golden/start-mist'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_THEME = 'mist'; MW_WINDOW_SIZE = '1440x1000' } },
    @{ Name = 'start-sand'; Dir = 'shots/golden/start-sand'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_THEME = 'sand'; MW_WINDOW_SIZE = '1440x1000' } },
    # SE-04：最近文件里有一条指向已删文件。**行留着**、标题转 secondary、副行「文件已移动」
    #   （`docs/30` §3.4）；点它走打开失败弹窗，弹窗里才有「从最近列表移除」。
    @{ Name = 'start-missing'; Dir = 'shots/golden/start-missing'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_HOME = '1'; MW_START_DEMO = '1'; MW_RECENTS_MISSING = '1'; MW_WINDOW_SIZE = '1440x1000' } },
    @{ Name = 'tabs'; Dir = 'shots/golden/tabs'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_TABS_DEMO = '1'; MW_WINDOW_SIZE = '1080x720' } },
    @{ Name = 'tabs-overflow'; Dir = 'shots/golden/tabs-overflow'; Ppp = '1.5'; Warmup = 30; Env = @{ MW_TABS_DEMO = '9'; MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-overflow.txt' } },
    @{ Name = 'tabs-wheel'; Dir = 'shots/golden/tabs-wheel'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_TABS_DEMO = '9'; MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-wheel.txt' } },
    @{ Name = 'tabs-wheel-line'; Dir = 'shots/golden/tabs-wheel-line'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_TABS_DEMO = '9'; MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-wheel-lines.txt' } },
    @{ Name = 'tabs-modal-input'; Dir = 'shots/golden/tabs-modal-input'; Ppp = '1.5'; Warmup = 24; Env = @{ MW_TABS_DEMO = '9'; MW_WINDOW_SIZE = '640x460'; MW_DIALOG_DEMO = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-modal-input.txt' } },
    @{ Name = 'opening'; Dir = 'shots/golden/opening'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_OPENING_DEMO = '1'; MW_WINDOW_SIZE = '960x680' } },
    @{ Name = 'open-error'; Dir = 'shots/golden/open-error'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_OPEN_FAILED = '1'; MW_WINDOW_SIZE = '960x680' } },
    @{ Name = 'shell-narrow'; Dir = 'shots/golden/shell-narrow'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_WINDOW_SIZE = '640x460'; MW_GOLDEN_INPUT = 'shots/golden/inputs/home.txt' } },
    @{ Name = 'shell-ink-compact'; Dir = 'shots/golden/shell-ink-compact'; Ppp = '1.5'; Warmup = 16; Env = @{ MW_THEME = 'ink'; MW_WINDOW_SIZE = '1080x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/home.txt' } },
    # M 轨概念图（CM-02）：浅色圆角节点 + 直线箭头 + 短语胶囊
    @{ Name = 'cm-default'; Dir = 'shots/golden/cm-default'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-narrow'; Dir = 'shots/golden/cm-narrow'; Ppp = '1.5'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '640x460' } },
    # M 轨概念图（CM-04）：「整理布局」后的同一张示例图，与 cm-default 构成前后对比
    @{ Name = 'cm-arrange'; Dir = 'shots/golden/cm-arrange'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_ARRANGE = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # M 轨概念图（CM-05）：补两条交叉连接 + 用 Shift+方向把选中概念右下各挪 24/16px。
    # 输入脚本 17 帧，warmup 必须覆盖它，否则截到的是脚本跑到一半的画面。
    @{ Name = 'cm-crosslink'; Dir = 'shots/golden/cm-crosslink'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CROSS = '1'; MW_CONCEPT_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-crosslink.txt' } },
    # M 轨概念图（CM-06）：概念绑卡片后的卡片页——锁「来源 概念图 · 用户痛点」这一行
    # 与顶栏「回到概念」按钮；`cm-card-link-back` 是同一份数据的回跳结果（选中态）。
    @{ Name = 'cm-card-link'; Dir = 'shots/golden/cm-card-link'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CARD = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-card-link-back'; Dir = 'shots/golden/cm-card-link-back'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CARD = '1'; MW_CONCEPT_CARD_BACK = '用户痛点'; MW_WINDOW_SIZE = '1280x800' } },
    # M 轨概念图（CM-07）：命题网按 `relationships` 生成树展开成全屏大纲——锁页头标注
    # 「概念图 · 按链接层级展开」、行首链接短语（`解决 →`）、页脚「N 个概念」。
    @{ Name = 'cm-outline'; Dir = 'shots/golden/cm-outline'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 M5（拖拽落点指示器）：同一手势、同一源节点「平移缩放」，只改落点。
    #   整框 2px 描边 + 15% 底 = 成为子级；上缘 3px 条 = 插到它之前；下缘 3px 条 = 插到它之后。
    #   `mm-drag-illegal` 换源节点「纯 Rust 原生」丢进它自己的子级「winit + windows-rs」，
    #   里面 2px 描边转红（224,75,75）。脚本以「按住不放」结束，截图时幽灵与落点都还在。
    # 三张强调色像素：child (988..1065, 608..652 边框)、before y 606..610、after y 653..655。
    @{ Name = 'mm-drag-child'; Dir = 'shots/golden/mm-drag-child'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-drag-child.txt' } },
    @{ Name = 'mm-drag-before'; Dir = 'shots/golden/mm-drag-before'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-drag-before.txt' } },
    @{ Name = 'mm-drag-after'; Dir = 'shots/golden/mm-drag-after'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-drag-after.txt' } },
    @{ Name = 'mm-drag-illegal'; Dir = 'shots/golden/mm-drag-illegal'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-drag-illegal.txt' } },
    # 打磨轮 M1/M2（新建节点后的编辑态，`docs/07` §3.2）：`Tab` 建子、`Enter` 建同级，
    #   都要「提交当前这颗 + 立刻在下一条开编辑」。像素上要能看到的是三件事：
    #   新盒子压在父节点右侧（990,641 起）、外面一圈强调色编辑环（3px，画在盒外留 2px 空隙）、
    #   `mm-new-cancel` 那张同位置什么都没有（Esc 把这次编辑建出来的节点一起收回去）。
    @{ Name = 'mm-new-child'; Dir = 'shots/golden/mm-new-child'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-new-child.txt' } },
    @{ Name = 'mm-new-chain'; Dir = 'shots/golden/mm-new-chain'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-new-chain.txt' } },
    @{ Name = 'mm-new-cancel'; Dir = 'shots/golden/mm-new-cancel'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-new-cancel.txt' } },
    @{ Name = 'mm-enter-sibling'; Dir = 'shots/golden/mm-enter-sibling'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-enter-sibling.txt' } },
    # 打磨轮 5（M8）：撤销节奏。四张同一份脚本的分叉——「Tab、打字」连建三颗之后
    # 分别停在：三颗都在 / 撤一次 / 撤三次 / 撤三次再重做一次。
    # 判据是「新建 + 写名字合成一条撤销」：撤一次整颗消失（不是只剩空盒子），
    # 撤三次正好回到一颗没建的时候，重做一次甲连名字一起回来。
    @{ Name = 'mm-undo-built'; Dir = 'shots/golden/mm-undo-built'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-undo-built.txt' } },
    @{ Name = 'mm-undo-one'; Dir = 'shots/golden/mm-undo-one'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-undo-one.txt' } },
    @{ Name = 'mm-undo-three'; Dir = 'shots/golden/mm-undo-three'; Ppp = '1.0'; Warmup = 32; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-undo-three.txt' } },
    @{ Name = 'mm-redo-one'; Dir = 'shots/golden/mm-redo-one'; Ppp = '1.0'; Warmup = 34; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-redo-one.txt' } },
    # 打磨轮 7（M6 键盘移动的连续节奏）：三张一组，只差「选了几颗」和「镜头在哪儿」。
    #   `mm-move-keys`：点一颗，Alt+↓ 三次、Alt+↑ 一次——每次只挪一格，节点在左右两侧交替换位。
    #   `mm-move-multi`：框选左侧两颗（marquee 轨迹 `picked=2`），Alt+↓ 一次整组下移一格、
    #     一条批量命令（轨迹 `structure move=1 selected=2 path=batch`）。
    #     打磨轮 60 复核：旧脚本 (600,430)→(780,640) 实际只框到根「织思」，Alt+↓ 被
    #     `move_block_reason=Root` 拦住、旧基线锁的是「静默不动」；已改框两颗可动子主题。
    #   `mm-move-keys-reveal`：中键把镜头拖走（主选滚到 (-716,-428)，整颗在视口外），再 Alt+↓——
    #     改之前这一步不跟镜头，节点留在画面外；改之后 `reveal_node` 把它拉回 (48,96) 的 48px 安全边内。
    #     注意两句里的数字都是本场景的实测：拖得不够远时节点仍在视口顶端可见，这张基线就锁不住行为。
    @{ Name = 'mm-move-keys'; Dir = 'shots/golden/mm-move-keys'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-move-keys.txt' } },
    @{ Name = 'mm-move-multi'; Dir = 'shots/golden/mm-move-multi'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-move-multi.txt' } },
    @{ Name = 'mm-move-keys-reveal'; Dir = 'shots/golden/mm-move-keys-reveal'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-move-keys-reveal.txt' } },
    # 打磨轮 8（M9 折叠圆点）：四张一组，钉住「动画有中间帧」和「热区比画出来的环大一点」。
    #   `mm-collapse`：点「纯 Rust 原生」的圆点（屏幕 (522,429)）→ 2 个子节点收起、圆点变＋。
    #   `mm-collapse-mid`：同一段脚本 + `MW_MOTION_T=0.5`，定格在过渡中点——
    #     消失的两颗正滑向父节点、半透明，与终态差 27913 px（1.6%），证明动画不是瞬间跳变。
    #   `mm-toggle-edge`：点圆心下方 9px（画出来的环是 r8，命中半径 r10）→ 仍然折叠，热区比环大。
    #   `mm-toggle-miss`：点圆心下方 12px → 什么都不命中（`canvas_click hit=none`），不误折叠。
    #   注意：坐标按第 19 轮之后**落定的相机**（zoom 1.33）量；旧坐标 (623,473) 在改动后一直
    #   点在空白处，两张「折叠」基线其实锁的是「没折」——2026-10-31 打磨轮 37 修正。
    @{ Name = 'mm-collapse'; Dir = 'shots/golden/mm-collapse'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-collapse.txt' } },
    @{ Name = 'mm-collapse-mid'; Dir = 'shots/golden/mm-collapse-mid'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-collapse-mid.txt'; MW_MOTION_T = '0.5' } },
    # X21（Stage C 第一包）：Ctrl+滚轮 = 以指针为锚缩放。两颗锚点各钉一半：
    #   `mm-zoom-wheel`：`mods ctrl` 后两格滚轮 → 缩放条读数与画布同步放大到 200% 以上，
    #     轨迹 `zoom_anchor src=wheel at=(700,452) world=(-158.63,-38.31) drift=(0.00,0.00)`
    #     在同一世界点重复出现 = 「锚点钉在指针下」的机器证据（drift 非零即漂）。
    #   若哪天滚轮又被读成平移，这张基线会整幅漂（视角不变、只差缩放条读数）。
    @{ Name = 'mm-zoom-wheel'; Dir = 'shots/golden/mm-zoom-wheel'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-zoom-wheel.txt' } },
    @{ Name = 'mm-toggle-edge'; Dir = 'shots/golden/mm-toggle-edge'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-toggle-edge.txt' } },
    @{ Name = 'mm-toggle-miss'; Dir = 'shots/golden/mm-toggle-miss'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-toggle-miss.txt' } },
    # 打磨轮 37（M17 悬停反馈）：本门一张「只差指针停在哪儿」的对照——
    #   `mm-toggle-hover`：指针停在折叠圆点上（(522,429)），圆点按 1.3× 放大并加环；
    #     与 `mm-toggle-miss`（同一帧、指针在 12px 外）差 377 px，全落在圆心周围 24×24 盒内。
    #   概要/边界的悬停对照在 cm 门（`mm-summary-hover` / `mm-boundary-hover`）。
    @{ Name = 'mm-toggle-hover'; Dir = 'shots/golden/mm-toggle-hover'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-toggle-hover.txt' } },
    # 打磨轮 46（M23 插入关系线）：两张一组，钉住「预览随指针」与「一步落地」。
    #   `mm-rel-link`：点「中文排版」(1167,503) → `Ctrl+L`（轨迹 `rel_link_mode`）→ 指针移到
    #     「交互与性能」(626,634)：源/目标各描 2px 强调环、虚线从源中心跟到指针、提示条说明 Esc 取消。
    #   `mm-rel-link-created`：同一段插入态，点目标 → 恰好一条 `AddRelationship`：
    #     轨迹 `rel_link_created` + `select relationship`，新线出现、标题栏变「未保存」、
    #     提示条换成「已插入关系线 · Ctrl+Z 撤销」。
    #   坐标按落定相机量（与 mm-collapse 同一套）；轨迹里的 `scene_node` 是首帧相机，别拿来点。
    @{ Name = 'mm-rel-link'; Dir = 'shots/golden/mm-rel-link'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-rel-link.txt' } },
    @{ Name = 'mm-rel-link-created'; Dir = 'shots/golden/mm-rel-link-created'; Ppp = '1.0'; Warmup = 22; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-rel-link-created.txt' } },
    # 打磨轮 49（M26 选中互斥）：两张一组，钉住「关系线选中道」被别的道接手时真的让位。
    #   `mm-rel-select-arrow`：建线（自动选中关系线）→ ArrowRight ×2：轨迹
    #     `selection_clear lane=relationship reason=navigate`；截图里 3px 强调线回到未选中态，
    #     只剩方向键落到的节点环与节点底。改前这两道是并存的（`shots/x100/M26/before-arrow.png`）。
    #   `mm-rel-select-delete`：同一段再按 Delete：轨迹 `delete_dispatch lane=nodes` +
    #     `delete-confirm pending=4 selected=1`，**没有** `delete relationship`；关系线仍在画面上。
    #   改前一次 Delete 跨两道：线当场消失，同时立起 4 节点删除确认（`before-delete.png`）。
    @{ Name = 'mm-rel-select-arrow'; Dir = 'shots/golden/mm-rel-select-arrow'; Ppp = '1.0'; Warmup = 22; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-rel-select-arrow.txt' } },
    @{ Name = 'mm-rel-select-delete'; Dir = 'shots/golden/mm-rel-select-delete'; Ppp = '1.0'; Warmup = 22; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-rel-select-delete.txt' } },
    # 打磨轮 47（M24 边界/概要键盘路径）：两张一组，钉住 `Ctrl+B` / `Ctrl+]` 真的建出注解。
    #   `mm-boundary-key`：点「禁则换行」(1409,503) → `Ctrl+B`：轨迹
    #     `annotation_add kind=boundary targets=1 parent=…`，边界圈住这一颗，标题栏变「未保存」。
    #   `mm-summary-key`：点「禁则换行」→ Shift 点「字形图集」(1409,586) → `Ctrl+]`：轨迹
    #     `annotation_add kind=summary targets=2 parent=…`，概要花括号盖住两颗。
    #   坐标按落定相机（zoom 1.33）量；首帧 `scene_node` 还是 fit 前的位置，别拿来点。
    @{ Name = 'mm-boundary-key'; Dir = 'shots/golden/mm-boundary-key'; Ppp = '1.0'; Warmup = 22; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-boundary-key.txt' } },
    @{ Name = 'mm-summary-key'; Dir = 'shots/golden/mm-summary-key'; Ppp = '1.0'; Warmup = 26; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-summary-key.txt' } },
    # 打磨轮 38（M18 Ctrl+X 删除确认）：VERIFY 而不是改——`HANDOFF` §7.5③ 说「剪切直接走」，
    #   读码看 `cut_action` / `apply_clipboard(Cut)` 都走 `delete_selected()`，带子节点会挂起确认。
    #   `mm-cut-confirm`：点「目标」（有 2 个子节点）→ `Ctrl+X`：轨迹应为
    #     `delete-confirm pending=3 selected=1` + `dialog id="canvas-delete"`；确认框带全屏遮罩，
    #     与 `mm-cut-cancel`（同一个脚本 + Esc）整屏不同；后者与「只选中不剪切」逐像素 0 px。
    #   `mm-cut-cancel`：Esc 取消后与只选中态 0 px，证明剪切没有在确认前偷偷删节点。
    @{ Name = 'mm-cut-confirm'; Dir = 'shots/golden/mm-cut-confirm'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-cut-confirm.txt'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'mm-cut-cancel'; Dir = 'shots/golden/mm-cut-cancel'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-cut-cancel.txt'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 9（M7 框选）：橡皮筋本身第一次有图，而且「松手会选中谁」在松手前就看得见。
    #   `mm-marquee`：从空白处按下拖到 (720,560)、**不松手**定格——虚线框 + 框内四个节点的选中环。
    #     轨迹 `marquee preview picked=… selected=…` 每变一次记一行。
    #   `mm-marquee-add`：先点一颗，再按住 Ctrl 框同一片——加选后共 4 颗（框里那颗本来就在组里）。
    @{ Name = 'mm-marquee'; Dir = 'shots/golden/mm-marquee'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-marquee.txt' } },
    @{ Name = 'mm-marquee-add'; Dir = 'shots/golden/mm-marquee-add'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-marquee-add.txt' } },
    # SE-05 检查器 v2（`docs/30` §4.1/§4.2）：288 默认宽 + 选中一颗节点。
    #   轨迹 `inspector w=288.00 head=40 obj=44 seg=30 bar=1.5 group=28 gutter=12` 报的是布局里的真值，
    #   不是从像素反推的——行高改了这张基线会漂，轨迹也会立刻改口。
    @{ Name = 'inspector'; Dir = 'shots/golden/inspector'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720' } },
    # 面板拖到下限 240：分段/色板/分组标题都不许顶宽。（修之前「形状」四段写死 66×4，
    # 内容最小宽 292，面板根本缩不到 240，用户拖到 240 下次打开又变回 293。）
    @{ Name = 'inspector-narrow'; Dir = 'shots/golden/inspector-narrow'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_INSPECTOR_W = '240'; MW_WINDOW_SIZE = '1280x720' } },
    # 没选中任何节点（§4.3）：一句话 + 文字动作「选中第一个主题」，上面不铺属性控件。
    @{ Name = 'inspector-empty'; Dir = 'shots/golden/inspector-empty'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x720' } },
    # SE-06 样式组（`docs/30` §4.2）。「形状与文字」默认折叠，上面三张永远拍不到
    #   边框/边宽/形状/字号——`MW_INSPECTOR_GROUPS` 启动即预置分组开合（只写内存不落盘，
    #   理由见 `WindowState::seed_inspector_group`），把颜色收起来、把形状那组打开。
    #   判据：边框色板 8 列 ×24、边宽与字号是 24 高 72 宽的 stepper、形状四段不顶宽。
    @{ Name = 'inspector-shape'; Dir = 'shots/golden/inspector-shape'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1' } },
    # 检查器四主题：色板/文字/边框的颜色全走主题令牌，换主题就得整块重画。
    @{ Name = 'inspector-ink'; Dir = 'shots/golden/inspector-ink'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_THEME = 'ink'; MW_WINDOW_SIZE = '1280x720' } },
    @{ Name = 'inspector-mist'; Dir = 'shots/golden/inspector-mist'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_THEME = 'mist'; MW_WINDOW_SIZE = '1280x720' } },
    @{ Name = 'inspector-sand'; Dir = 'shots/golden/inspector-sand'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_THEME = 'sand'; MW_WINDOW_SIZE = '1280x720' } },
    # SE-07 内容组（`docs/30` §4.1/§4.2 / §4.6 的 `inspector-content`）：页签停在「内容」这一页，
    #   五组默认展开，节点上喂齐五项内容（`MW_CONTENT_DEMO` 全部走 `Command`）。
    #   判据：备注框 3 行起、标签是 6px 色点 + 纯文字（没有胶囊）、任务三态有选中、
    #   链接显示 host、卡片显示标题 + 来源。
    @{ Name = 'inspector-content'; Dir = 'shots/golden/inspector-content'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_INSPECTOR_TAB = 'note'; MW_CONTENT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720' } },
    # SE-08 作用域（`docs/30` §4.2/§4.6 的 `inspector-multiselect`）：三颗节点一起选中，
    # 对象行是「3 个节点」，样式组标题右侧是「本节点 / 含子树」两段（旧版是页顶单独一行「作用」）。
    @{ Name = 'inspector-multiselect'; Dir = 'shots/golden/inspector-multiselect'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = 'multi'; MW_WINDOW_SIZE = '1280x720' } },
    # SE-09 错误条（`docs/30` §4.3「命令失败：面板内 28px 错误条 + toast」）：走一次真的失败路径——
    # 发一条指向不存在节点的 SetStyle，壳层照用户遇到失败那样贴条（`MW_INSPECTOR_FAIL=1`）。
    @{ Name = 'inspector-error'; Dir = 'shots/golden/inspector-error'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_INSPECTOR_FAIL = '1'; MW_WINDOW_SIZE = '1280x720' } },
    # SE-06b 数值格键入（`docs/30` §4.4「每个可改属性都有键盘路径」）：字号那格摆成编辑态，
    # 草稿 `48` 已经敲进去、下面一条强调色底线；其余行与 `inspector-shape` 完全一致。
    @{ Name = 'inspector-stepper-edit'; Dir = 'shots/golden/inspector-stepper-edit'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1'; MW_STEPPER_EDIT = 'inspector/style/size:48' } },
    # SE-13 四行新控件（字重 / 对齐 / 圆角 / 椭圆，`docs/30` §4.2 必选能力清单的最后四项）：
    #   `MW_STYLE_DEMO=<k=v,…>` 经 `Command::SetStyle` 给「纯 Rust 原生」喂一份覆盖，
    #   画布与面板同时进入「该值真的生效」的状态——离屏每帧清指针，点不到控件。
    #   四张各只开一个维度，漂移能一眼落到位：字重换字面（盒宽跟着变）、对齐挪锚点、
    #   圆角 24 顶格、椭圆换形状（负半径哨兵）。
    @{ Name = 'inspector-weight'; Dir = 'shots/golden/inspector-weight'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1'; MW_STYLE_DEMO = 'weight=600' } },
    @{ Name = 'inspector-align'; Dir = 'shots/golden/inspector-align'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1'; MW_STYLE_DEMO = 'align=right' } },
    @{ Name = 'inspector-radius'; Dir = 'shots/golden/inspector-radius'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1'; MW_STYLE_DEMO = 'radius=24' } },
    @{ Name = 'inspector-ellipse'; Dir = 'shots/golden/inspector-ellipse'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_INSPECTOR = '1'; MW_SELECT_DEMO = '1'; MW_WINDOW_SIZE = '1280x720'; MW_INSPECTOR_GROUPS = 'style/color=0,style/shape=1'; MW_STYLE_DEMO = 'shape=ellipse' } },
    # 打磨轮 41（M19 搜索命中备注/链接）：同一份样例文档、同一脚本结构，只换查询词。
    #   `MW_SEARCH_DEMO=1` 给「纯 Rust 原生」经 `Command` 写入备注（含 html）与链接（https）；
    #   `MW_NOTES=none` 关掉本机卡片库，结果只来自文档本身；隔离 APPDATA 由 Run-GoldenScene 统一给。
    #   判据：首行是命中处摘录（HTML/https 暖黄底）、右侧角标「备注」/「链接」、
    #   第二行「节点 · 纯 Rust 原生 · 织思」；轨迹同帧写 `search_hits q=… rows=1 note=… link=…`。
    @{ Name = 'search-note'; Dir = 'shots/golden/search-note'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_SEARCH_DEMO = '1'; MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-note.txt' } },
    @{ Name = 'search-link'; Dir = 'shots/golden/search-link'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_SEARCH_DEMO = '1'; MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-link.txt' } },
    # 打磨轮 52（M29 匹配方式可切换）：`zw` 是「中文排版」的首字母——拼音模式命中 1 行，
    #   `Ctrl+Shift+M` 切「精确文本」后结果归零，脚注右侧两段开关高亮在「精确文本」。
    #   键位不能用 `Ctrl+P`/`Ctrl+Shift+P`：全局 `Ctrl+P` 是「导出 PDF」，且 egui 的逻辑匹配
    #   允许额外 Shift，两种写法都会被 `window_shortcuts` 先吃掉；`M` 没有全局 `Ctrl+M`。
    #   判据：轨迹 `search_mode mode=text rows=0` + `search_hits q=zw rows=0`；
    #   空状态第二行改成「精确文本只匹配原文…」。脚本 33 帧，warmup 由 harness 自动覆盖。
    @{ Name = 'search-text'; Dir = 'shots/golden/search-text'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-text.txt' } },
    # 打磨轮 43（M20 搜索跳进折叠子树）：`MW_COLLAPSE_DEMO=中文排版` 启动即折叠该节点，
    #   让「禁则换行」只在检索里存在（画布场景排不到）。脚本 Ctrl+F → 输入 → Enter 跳转。
    #   判据：轨迹 `search_reveal … expanded=1` + `search_jump … ok=1`；画布展开「中文排版」
    #   三颗子主题、选中环落在「禁则换行」、相机居中；搜索卡片不动（区域 0 像素差）。
    #   warmup 44 = 33 帧脚本 + 余量，与手工拍基线时一致，避免两个路径的预热差。
    @{ Name = 'search-collapsed'; Dir = 'shots/golden/search-collapsed'; Ppp = '1.0'; Warmup = 44; Env = @{ MW_COLLAPSE_DEMO = '中文排版'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-collapsed.txt' } },
    # 打磨轮 44（M21 Ctrl+H 逐个替换与预览）：同一份样例、同一条「Ctrl+F → 排版 → Enter」开头，
    #   只多展开替换行。`search-replace` 停在「替换框有字、当前行第二行 → 中文排布」；
    #   `search-replace-one` 再按 Ctrl+Enter，标题变「中文排布」、相机居中、轨迹 `search_replace_one`，
    #   搜索条与替换行都还开着（焦点不跳回画布），结果归零后显示「无结果」。
    @{ Name = 'search-replace'; Dir = 'shots/golden/search-replace'; Ppp = '1.0'; Warmup = 58; Env = @{ MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-replace.txt' } },
    @{ Name = 'search-replace-one'; Dir = 'shots/golden/search-replace-one'; Ppp = '1.0'; Warmup = 68; Env = @{ MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-replace-one.txt' } },
    # 打磨轮 45（M22 聚焦视图外的搜索跳转）：`MW_FOCUS_DEMO=中文排版` 启动即聚焦该子树，
    #   脚本 Ctrl+F → wgpu → Enter。判据：轨迹 `search_focus_exit … levels=1` + `search_jump ok=1`
    #   + toast「已退出聚焦并跳转」；画布退出聚焦回到全图、选中「wgpu 四层合成器」并居中，
    #   搜索卡片保持原样（区域 0 像素差）。warmup 48 = 37 帧脚本 + 余量，与手工拍基线一致。
    @{ Name = 'search-focus-jump'; Dir = 'shots/golden/search-focus-jump'; Ppp = '1.0'; Warmup = 48; Env = @{ MW_FOCUS_DEMO = '中文排版'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-focus-jump.txt' } },
    # 打磨轮 55（M30 缩略卡接回画布）：出厂关，只有 `MW_MINIMAP=1` 才画——既有基线因此零漂移。
    #   两张同夹具同窗口（1280×800）：展开态 190×130、折叠态 34×26，两者同右对齐、同底边
    #   （缩放条上方 8px）。判据是右下角浮层区域 + `minimap` 轨迹，不是「看起来像」。
    @{ Name = 'mm-minimap'; Dir = 'shots/golden/mm-minimap'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_MINIMAP = '1'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'mm-minimap-collapsed'; Dir = 'shots/golden/mm-minimap-collapsed'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_MINIMAP = '1'; MW_MINIMAP_COLLAPSED = '1'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 57（M29b 搜索脚注窄窗）：640×700 + 检查器 288 → 中央 383、卡片 359（<420），
    #   匹配方式开关只显示「文本 / 拼音」；判据是轨迹 `search_footer width=359 narrow=1`，
    #   纯函数单测钉住「左留 64 给提示、右留 74 给 Esc」。ppp 1.5 才能让逻辑宽 = 脚本里的 640。
    @{ Name = 'search-narrow'; Dir = 'shots/golden/search-narrow'; Ppp = '1.5'; Warmup = 8; Env = @{ MW_WINDOW_SIZE = '640x700'; MW_INSPECTOR = '1'; MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-narrow.txt' } },
    # 打磨轮 58（M30b 缩略卡双击定位）：双击卡内一颗节点 → 选中 + `reveal_id` 居中；
    #   双击空白 → `minimap locate hit=0`，不选中、相机按拖视口框走。
    @{ Name = 'mm-minimap-locate'; Dir = 'shots/golden/mm-minimap-locate'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_MINIMAP = '1'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-minimap-locate.txt' } },
    # 打磨轮 60（M6b 结构移动边界反馈）：点「uniform grid 命中」（第一个子主题）→ Alt+↑，
    #   文档不变、只多一条提示条；轨迹 `structure_move_blocked … reason=First`。
    #   与「不发键」的对照帧只差提示条区域（10 760 px，bbox x[16..375] y[1028..1057]）。
    @{ Name = 'mm-move-blocked'; Dir = 'shots/golden/mm-move-blocked'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-move-blocked.txt' } },
    # 打磨轮 62（M7b 套索与 Alt 减选）：Alt+空白拖 = 自由套索（且恒为减选）。
    #   `mm-lasso` 停在松手之前，锁多边形正在画的样子；`mm-marquee-alt` 先框 3 颗再套索减 1 颗，
    #   末帧带 2 个选中环。轨迹 `marquee picked=… selected=… path=lasso`。
    @{ Name = 'mm-lasso'; Dir = 'shots/golden/mm-lasso'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-lasso.txt' } },
    @{ Name = 'mm-marquee-alt'; Dir = 'shots/golden/mm-marquee-alt'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-marquee-alt.txt' } },
    # 打磨轮 64（M31 设为根主题）：右键「中文排版」→ 菜单第 12 行「设为根主题」（Home + 11×↓），
    #   旧根「织思」整棵树挂到它下面、取景按新根重新 fit。轨迹 `set-root id=… ok=1`。
    @{ Name = 'mm-set-root'; Dir = 'shots/golden/mm-set-root'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-set-root.txt' } },
    # 打磨轮 65（M32 双击折叠圆点「展开到下一级」）：`MW_DRILL_DEMO` 在「纯 Rust 原生」的
    #   第一颗子级下挂「GPU 合成」（只在设了环境变量时生效，默认样例不变）。
    #   `mm-toggle-drill`：单击折叠 → 双击 → 只露直接子级（自己展开、有下一层的子级折回）；
    #   `mm-toggle-drill-deep`：再双击一次 → 子级展开，第三层「GPU 合成」可见。
    #   轨迹 `toggle_drill id=… depth=1 open=1` 与 `depth=1 open=0`。
    @{ Name = 'mm-toggle-drill'; Dir = 'shots/golden/mm-toggle-drill'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_NOTES = 'none'; MW_DRILL_DEMO = '纯 Rust 原生'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-toggle-drill.txt' } },
    @{ Name = 'mm-toggle-drill-deep'; Dir = 'shots/golden/mm-toggle-drill-deep'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_NOTES = 'none'; MW_DRILL_DEMO = '纯 Rust 原生'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-toggle-drill-deep.txt' } },
    # 打磨轮 67（M9b 整图折叠与聚焦视图下的圆点，账本 §3 第 16 项）：M9 的四张拍的都是
    #   默认展开态的单颗圆点，`Ctrl+/`（`SetCollapsedAll`）与 `Alt+Enter`（`focus_selected`）
    #   这两种整体状态下的圆点从没拍过。本轮不改行为，只补画面证据。
    #   `mm-collapse-all-dots`：整图折叠终态——可见的带下级节点全是「＋」，根不画圆点；
    #   `mm-focus-dots`：`MW_DRILL_DEMO` 先给「winit + windows-rs」挂一颗孙子，
    #     再 `MW_FOCUS_DEMO=纯 Rust 原生` 聚焦该子树——聚焦视图**内部**的边界圆点要还在，
    #     聚焦根本身不画圆点（它在裁剪后的场景里没有父级）。
    @{ Name = 'mm-collapse-all-dots'; Dir = 'shots/golden/mm-collapse-all-dots'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_NOTES = 'none'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-collapse-all-dots.txt' } },
    @{ Name = 'mm-focus-dots'; Dir = 'shots/golden/mm-focus-dots'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_NOTES = 'none'; MW_DRILL_DEMO = '纯 Rust 原生'; MW_FOCUS_DEMO = '纯 Rust 原生'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-focus-dots.txt' } }
)

function Run-GoldenScene($scene) {
    $app = Join-Path $root "target/ci-appdata/gate-$($scene.Name)"
    $localApp = "$app-local"
    New-Item -ItemType Directory -Force -Path $app | Out-Null
    New-Item -ItemType Directory -Force -Path $localApp | Out-Null
    $saved = @{}
    $all = @{ APPDATA = $app; LOCALAPPDATA = $localApp }
    foreach ($k in $scene.Env.Keys) { $all[$k] = $scene.Env[$k] }
    foreach ($k in $all.Keys) {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process')
        [Environment]::SetEnvironmentVariable($k, [string]$all[$k], 'Process')
    }
    try {
        $extra = @()
        if ($Update) { $extra += '--update' }
        $out = & cargo golden --ppp $scene.Ppp --tol 0 --warmup $scene.Warmup --dir $scene.Dir @extra 2>&1
        foreach ($line in $out) { Say ([string]$line) }
        $sceneCode = [int]$LASTEXITCODE
        return $sceneCode
    } finally {
        foreach ($k in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process')
        }
    }
}

Say "X100 Stage B machine gate (X11-X20)"
if ($Update) { Say 'MODE: re-shoot baselines (--update) — 外观有意改动，旧图请在报告里登记' }
Say "root=$root"
Say "started=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$null = Run-Step 'X11-X19 evidence manifests' {
    $code = 0
    foreach ($i in 11..19) {
        $round = 'X{0:D2}' -f $i
        & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-evidence.ps1 -Round $round -Check
        if ($LASTEXITCODE -ne 0) { $code = $LASTEXITCODE }
    }
    if ($code -ne 0) { throw "evidence check failed: $code" }
}

$null = Run-Step 'PowerShell syntax' {
    foreach ($file in Get-ChildItem scripts/x100-*.ps1) {
        $null = [scriptblock]::Create((Get-Content -Raw -Encoding UTF8 $file.FullName))
        Write-Output "syntax-ok $($file.Name)"
    }
}

$null = Run-Step 'cargo fmt --check' { cargo fmt --all -- --check }

if (-not $SkipClippy) {
    $null = Run-Step 'mw-shell clippy: no new X20 warnings' {
        $output = & cargo clippy -p mw-shell --all-targets 2>&1
        $code = $LASTEXITCODE
        $output | Set-Content -LiteralPath (Join-Path $logDir 'stage-b-clippy.log') -Encoding UTF8
        # Only warnings located in the files X20 touched are gated (start/menu/...).
        # export_pdf's three historical warnings are out of scope.
        # The `constructor `rgba` ...` warning in mw-core/theme.rs is historical too. It used to
        # be exempted by "file:line:col", so deleting one import above it shifted the line and
        # turned an old warning into a false "new" one (polish round 10). Exempt by title+file.
        $historicalTitle = 'constructor .rgba. has the same name as the type'
        $touched = @()
        $prev = ''
        foreach ($line in $output) {
            $text = [string]$line
            if ($text -match '--> .*(golden\.rs|theme\.rs|inspector_methods_2\.rs|inspector\.rs|start\.rs|menu\.rs|app_methods_2\.rs|tab_strip\.rs|main\.rs)') {
                $isHistorical = ($prev -match $historicalTitle) -and ($text -match 'mw-core\\src\\theme\.rs')
                if (-not $isHistorical) { $touched += $text }
            }
            if ($text.Trim() -ne '') { $prev = $text }
        }
        if ($touched.Count -gt 0) {
            $touched | ForEach-Object { Say "X20-warning $_" }
            throw 'new X20 clippy warning'
        }
        if ($code -ne 0) { throw "clippy failed: $code" }
    }
}

if (-not $SkipFullTests) {
    Say ""
    Say '==> full serial workspace tests'
    $output = & cargo test --workspace -- --test-threads=1 2>&1
    $code = $LASTEXITCODE
    $output | Set-Content -LiteralPath (Join-Path $logDir 'stage-b-tests.log') -Encoding UTF8
    $counts = Get-TestCounts $output
    foreach ($line in ($output | Select-Object -Last 20)) { Say ([string]$line) }
    Say "tests passed=$($counts.Passed) failed=$($counts.Failed) exit=$code"
    if ($code -ne 0 -or $counts.Failed -ne 0 -or $counts.Passed -lt 747) {
        $failures.Add("full tests (passed $($counts.Passed), failed $($counts.Failed), exit $code)")
        Say 'FAIL full serial workspace tests'
    } else {
        Say 'PASS full serial workspace tests'
    }
}

if (-not $SkipGolden) {
    cargo build --release -p mw-shell | Out-Host
    if ($LASTEXITCODE -ne 0) { $failures.Add('release build'); Say 'FAIL release build' }
    # 两遍连跑是刻意的：第 2 遍用来抓「自己和自己不一致」。但第 1 遍紧跟在 fmt/clippy/test 之后，
    # 机器还在降载，依赖真实时间的场景（滚动落定、悬停计时）会在这一遍多抓半帧——
    # 2026-10-28 连两次「第 1 遍失败 / 第 2 遍零差异」（先是 outline-20k-deep 0.496%，
    # 后是 outline-hover-select 2.751%）之后，这里先留 10 秒让负载落下来。
    Start-Sleep -Seconds 10
    foreach ($pass in 1..2) {
        $null = Run-Step "golden matrix pass $pass ($($scenes.Count) comparisons)" {
            $code = 0
            foreach ($scene in $scenes) {
                $sceneCode = [int](Run-GoldenScene $scene)
                if ($sceneCode -ne 0) {
                    Say "FAIL golden $($scene.Name)"
                    $code = $sceneCode
                } else {
                    Say "ok golden $($scene.Name)"
                }
            }
            if ($code -ne 0) { throw "golden drift: $code" }
        }
    }
}

Say ""
if ($failures.Count -gt 0) {
    Say "MACHINE GATE FAIL: $($failures -join '; ')"
    $code = 1
} else {
    Say 'MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED'
    $code = 0
}
Say "finished=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$logPath = Join-Path $logDir 'stage-b-gate.log'
$log | Set-Content -LiteralPath $logPath -Encoding UTF8
Write-Host "log=$logPath"
exit $code
