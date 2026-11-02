param(
    [switch]$SkipFullTests,
    [switch]$SkipGolden,
    [switch]$SkipClippy,
    # 重拍基线：每个场景照同一份环境表跑一遍 --update，第二遍就是对刚写下的
    # 基线做逐像素复核。只有「外观有意改动」时才用，日志里会写明。
    [switch]$Update
)

# G-CM-2 机器门（概念图 M2：CM-06..CM-08）。
# 覆盖：docs/30 §5.10 的 10 个验收场景 ×2 遍逐像素比较、`.zmind` 往返断言、
# fmt、mw-shell clippy 新增 0、全量串行测试、release 构建。
# 用法：powershell -File scripts/x100-stage-cm-gate.ps1
# 退出码：0 = 机器门通过（人工判词仍需用户给）；非 0 = 第一处失败。
#
# 与 scripts/x100-stage-b-gate.ps1 的边界：那份锁 B2/CER 的 38 组与起始页；
# 这份只锁 `cm-*`。两份互不覆盖对方的场景，互不引用对方的基线。
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# 测试会往 $env:TEMP 写临时目录（`.zmind` 往返用例）。在受限令牌下系统 Temp 可能不可写
# （本会话实测 `os error 5 拒绝访问`），测试就会「假失败」。统一把 TEMP/TMP 指到仓库内
# `target\tmp`：测试不再依赖用户 Temp 的权限，临时垃圾也留在 target 里。
$repoTemp = Join-Path $root 'target\tmp'
New-Item -ItemType Directory -Force -Path $repoTemp | Out-Null
$env:TEMP = (Resolve-Path $repoTemp).Path
$env:TMP = $env:TEMP
$logDir = Join-Path $root 'target/x100'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = New-Object System.Collections.Generic.List[string]
$failures = New-Object System.Collections.Generic.List[string]
$goldenSkipped = 0

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

# 概念图场景（现 62 条），与 docs/30 §5.10 一一对应（`cm-roundtrip` 是测试不是截图，
# 见下面的 `.zmind` 往返断言步骤）。
# `Warmup` 只按需要给：编辑态（结点编辑 / 短语编辑 / IME 组合）必须覆盖到
# 整个输入脚本落定之后，纯静态画面 6 帧足够。
$scenes = @(
    @{ Name = 'cm-default'; Dir = 'shots/golden/cm-default'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 42（C17）：概念图命题短语进检索。`MW_NOTES=none` 关掉本机笔记库，
    # 否则卡片正文里的「支撑」会多出一行、基线锁不住文档自身的命中。
    @{ Name = 'search-rel'; Dir = 'shots/golden/search-rel'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_CONCEPT_DEMO = '1'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-rel.txt' } },
    # 同一脚本 + Enter：选中该命题并居中到两端概念盒中点，搜索框不丢焦点（trace 无第二次 focus）。
    @{ Name = 'search-rel-jump'; Dir = 'shots/golden/search-rel-jump'; Ppp = '1.0'; Warmup = 38; Env = @{ MW_CONCEPT_DEMO = '1'; MW_NOTES = 'none'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/search-rel-jump.txt' } },
    @{ Name = 'cm-copy-paste'; Dir = 'shots/golden/cm-copy-paste'; Ppp = '1.0'; Warmup = 40; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-copy-paste.txt' } },
    # CM-10 ⑥：同一张示例图，写上聚焦问题——画布顶部那行字是这张基线要锁的东西。
    @{ Name = 'cm-focus-question'; Dir = 'shots/golden/cm-focus-question'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_FOCUS = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-10 ⑥：双击那条带子就地改问题——双击进编辑 → Shift+Home 选中整行 → 整句替换 → 回车提交。
    # 这一场是脚本化的指针路径：它暴露过三个真 bug（编辑器根本没被调用 / 双击连带新建浮动主题 / Shift+Home 顺手把画布适应视图）。
    @{ Name = 'cm-focus-question-edit'; Dir = 'shots/golden/cm-focus-question-edit'; Ppp = '1.0'; Warmup = 40; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_FOCUS = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-focus-question-edit.txt' } },
  # CM-10 ⑥：带子的**悬停态**——指针压上去会亮一层底色、加一条强调线、字也变深。
  # 这一场故意让指针停在带子上拍（其它脚本都要把指针挪开，否则 hover 会被当成假差异）。
@{ Name = 'cm-focus-question-hover'; Dir = 'shots/golden/cm-focus-question-hover'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_FOCUS = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-focus-question-hover.txt' } },
  # CM-10 ⑦：检查器那四格箭头的**指针路径**——Esc 关掉菜单后点第四格「双向」。
  # 这条脚本走的是完全没被覆盖过的一条路：四格接没接对命令，以前只能靠读代码。
  # 对照实验证明点击本身零痕迹：点「已经在用的那一格」与不点相比 0 px。
@{ Name = 'cm-rel-adjacent'; Dir = 'shots/golden/cm-rel-adjacent'; Ppp = '1.0'; Warmup = 16; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-rel-adjacent.txt' } },
@{ Name = 'cm-rel-curve'; Dir = 'shots/golden/cm-rel-curve'; Ppp = '1.0'; Warmup = 22; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK_BLANK = '增长策略|600|-500'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-rel-curve.txt' } },
@{ Name = 'cm-rel-hover'; Dir = 'shots/golden/cm-rel-hover'; Ppp = '1.0'; Warmup = 16; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-rel-hover.txt' } },
    # 打磨轮 39（C15）：同一对概念之间有两条平行命题时，两条车道各自独立可点。
    # 两张脚本分别点**不同车道**的短语（坐标来自 `scene_rel` trace 的 label= 字段），
    # trace 里 `select relationship` 的两条 id 必须不同——这就是「不是靠挑位置碰巧选中同一条」的证据。
    @{ Name = 'cm-parallel-select-a'; Dir = 'shots/golden/cm-parallel-select-a'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PARALLEL = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-parallel-select-a.txt' } },
    @{ Name = 'cm-parallel-select-b'; Dir = 'shots/golden/cm-parallel-select-b'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PARALLEL = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-parallel-select-b.txt' } },
    # 打磨轮 63（C21）：同一对概念 3 条命题。`MW_CONCEPT_PARALLEL=2` = 再补两条
    # （连同模板自带的「转化」共 3 条），等距避让只在 n ≥ 3 生效——没有这张图就没有像素证据。
    @{ Name = 'cm-parallel-3'; Dir = 'shots/golden/cm-parallel-3'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PARALLEL = '2'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-10 ⑦ 残余：拖关系线终点期间**不画** 3px 强调。两张图只差一个状态位，
    # 逐像素求交得出「只在选中态出现 1 081 px（强调）、只在拖拽态出现 7 140 px（预览线）」。
    @{ Name = 'cm-relink-select-only'; Dir = 'shots/golden/cm-relink-select-only'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_SELECT_REL = '带来'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-relink-drag'; Dir = 'shots/golden/cm-relink-drag'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_RELINK_DRAG = '带来|200|140'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 12（C3）：指针停在**目标盒外 6px**（吸附半径内），必须画出落点高亮框。
    # 修复前严格包含判定 → target=None → 预览画成落空（松手会新建概念）。
    @{ Name = 'cm-relink-near'; Dir = 'shots/golden/cm-relink-near'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_RELINK_DRAG = '带来|-243|-72'; MW_WINDOW_SIZE = '1280x800' } },
@{ Name = 'cm-arrow-inspector'; Dir = 'shots/golden/cm-arrow-inspector'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_ARROW_MENU = '1'; MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-arrow-inspector.txt' } },
    # 打磨轮 54（C19）：概念图检查器——概念属性、钉住开关与局部整理。
    # `cm-inspector`：选中「产品定位」（未钉住），展示概念元信息、无子树作用域的颜色/形状组、概念排布与自动重排摘要。
    # `cm-inspector-pinned`：选中已钉住的「产品定位」，展示「已钉住 · 概念」、开关置开与「已钉住」摘要。
    @{ Name = 'cm-inspector'; Dir = 'shots/golden/cm-inspector'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_SELECT = '产品定位'; MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-inspector-pinned'; Dir = 'shots/golden/cm-inspector-pinned'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_SELECT = '产品定位'; MW_CONCEPT_PIN = '产品定位'; MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-10 ⑦：端点箭头四档。不设 `MW_CONCEPT_ARROW` 就是「前向」——上面 `cm-default` 画的就是它，
    # 所以这一档不用新基线；下面三张各锁一档。
    @{ Name = 'cm-arrow-off'; Dir = 'shots/golden/cm-arrow-off'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_ARROW = 'off'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-arrow-backward'; Dir = 'shots/golden/cm-arrow-backward'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_ARROW = 'backward'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-arrow-both'; Dir = 'shots/golden/cm-arrow-both'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_ARROW = 'both'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-narrow'; Dir = 'shots/golden/cm-narrow'; Ppp = '1.5'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '640x460' } },
    @{ Name = 'cm-arrange'; Dir = 'shots/golden/cm-arrange'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_ARRANGE = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 13（C7）：整理布局后按**一次** Ctrl+Z（脚本 14 帧，warmup 20 吃满）。
    # 锁的是「整理丢了手工位置，一条撤销整张回来」；整理在撤销栈里只占一条（trace `undo label=摆放概念`）。
    @{ Name = 'cm-arrange-undo'; Dir = 'shots/golden/cm-arrange-undo'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_ARRANGE = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-arrange-undo.txt' } },
    # 打磨轮 53（C18）：概念图「钉住」概念与位置受保护。
    # `cm-pin`：钉住「技术方案」，右上角露出绿色圆点受保护微标；
    # `cm-arrange-pinned`：整理布局时，钉住的「技术方案」保持手工原位不动，其余概念环绕重排并避让。
    @{ Name = 'cm-pin'; Dir = 'shots/golden/cm-pin'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PIN = '技术方案'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-arrange-pinned'; Dir = 'shots/golden/cm-arrange-pinned'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PIN = '技术方案'; MW_CONCEPT_ARRANGE = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-crosslink'; Dir = 'shots/golden/cm-crosslink'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CROSS = '1'; MW_CONCEPT_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-crosslink.txt' } },
    @{ Name = 'cm-card-link'; Dir = 'shots/golden/cm-card-link'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CARD = '1'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-card-link-back'; Dir = 'shots/golden/cm-card-link-back'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_CARD = '1'; MW_CONCEPT_CARD_BACK = '用户痛点'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-outline'; Dir = 'shots/golden/cm-outline'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-10 ⑥：问题可以问得很长——画布那条带子按画布宽换行，不排成一行被窗口硬切。
    @{ Name = 'cm-focus-long'; Dir = 'shots/golden/cm-focus-long'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_FOCUS = '1'; MW_CONCEPT_FOCUS_TEXT = '优质睡眠由哪些日常习惯、作息时间、情绪起伏、卧室光照、咖啡因摄入、运动安排与睡前屏幕时间共同决定？而在这四十来条里，哪一条才是今晚最该先动、动了就能立刻感觉到差别的那一条？如果改一条就够，那它应该先改谁旁边的谁，才不至于让别的习惯白改一场、也不必等到明天夜里才知道有没有用？'; MW_WINDOW_SIZE = '1280x800' } },    # CM-10 ⑦：四档互斥，菜单里当前那档要打勾——否则点了「反向 ←」看不出变化。
    @{ Name = 'cm-arrow-menu'; Dir = 'shots/golden/cm-arrow-menu'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_ARROW = 'backward'; MW_CONCEPT_ARROW_MENU = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-10 ⑥：大纲页也要看到聚焦问题——问题带贴在页头下沿、正文列里居中。
    @{ Name = 'cm-outline-focus'; Dir = 'shots/golden/cm-outline-focus'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_CONCEPT_FOCUS = '1'; MW_WINDOW_SIZE = '1280x800' } },    # CM-10 ⑥：那条带子还能就地改——双击立起编辑框（与画布同一个控件、同一套键规则）。
    @{ Name = 'cm-outline-focus-edit'; Dir = 'shots/golden/cm-outline-focus-edit'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_CONCEPT_FOCUS = '1'; MW_CONCEPT_FOCUS_EDIT = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # 2026-10-27 04:45：⑦ 的**激活路径**——夹具展开子菜单后，键盘三下到「双向 ↔」并 Enter，
    # 文档真的从「无箭头」变成「双向」。这一步是 goldens 里第一次真正**选一行菜单条目**
    # （此前所有菜单场景都用键盘开菜单、再用快捷键，没有点过行）；指针点击在离屏里不派发，见 report §2.19。
    @{ Name = 'cm-arrow-menu-key'; Dir = 'shots/golden/cm-arrow-menu-key'; Ppp = '1.0'; Warmup = 32; Env = @{ MW_CONCEPT_ARROW = 'off'; MW_CONCEPT_ARROW_MENU = '1'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-arrow-menu-key.txt'; MW_WINDOW_SIZE = '1280x800' } }
    # 2026-10-27 04:40：CM-05 的「菜单里不许出现按不动的条目」先前只有代码依据。
    # 概念图空白处的菜单正好 9 项（3 条分隔线 + 新建概念 / 粘贴 / 整理布局 / 适应画布 / 100% / 打开文档所在文件夹），
    # **没有**「全部展开 / 全部折叠」（树语义），也不叫「新建自由主题」。
    @{ Name = 'cm-blank-menu'; Dir = 'shots/golden/cm-blank-menu'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONTEXT_DEMO = 'blank'; MW_WINDOW_SIZE = '1280x800' } }
    # 2026-10-27：多选那条 arm（`selected_count() > 1`）本来一条基线都没有，而且它带「排列」子菜单
    # （10 项、含两个分组标题）——正好压 `place_submenu` 的竖直夹取。
    @{ Name = 'cm-multi-menu'; Dir = 'shots/golden/cm-multi-menu'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONTEXT_DEMO = 'multi'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 4（C10）：同一张多选菜单，请求点挪到矮窗底部，「排列」子菜单（8 行 + 2 段标题
    # = 300 高）必然溢出底边，于是被 `place_submenu` 夹进来——这是 CM-10 §3 第 8 条第一次有图。
    # 夹取后子菜单会盖住父菜单下半部分（`menu.rs` 里写明取舍），横向只重叠 2px、父行仍可点。
    @{ Name = 'cm-submenu-bottom'; Dir = 'shots/golden/cm-submenu-bottom'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONTEXT_DEMO = 'multi'; MW_CONTEXT_POS = '380,400'; MW_WINDOW_SIZE = '1280x520' } }
    # 2026-10-27：⑦ 的**指针**路径。第 27 轮的负结果（「弹层行点不动」）已证实是坐标算错：
    # 当时点的 (410,122) 落在菜单卡顶部 8px 内边距里（第一行从 y=127 起）。
    # 这一场的终态与 cm-arrow-menu-key **逐像素相同**——键盘选行与指针点行落到同一个文档状态。
    @{ Name = 'cm-arrow-menu-click'; Dir = 'shots/golden/cm-arrow-menu-click'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_ARROW_MENU = '1'; MW_CONCEPT_ARROW = 'off'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-arrow-menu-click.txt' } }
    # 2026-10-27：拖动建线的**指针几何**。第 31 轮之前只有 MW_CONCEPT_LINK_BLANK 钩子版，
    # 「按在锚点上 → 拖 → 松在空白处」这段没有脚本证据。锚点坐标由一次性探针 MW_PROBE_NODES 量出。
    @{ Name = 'cm-link-drag-pointer'; Dir = 'shots/golden/cm-link-drag-pointer'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-link-drag-pointer.txt' } }
    # CM-08 新增三张：改名 / 新建连线（空短语编辑态）/ 短语 IME 组合。
    @{ Name = 'cm-node-edit'; Dir = 'shots/golden/cm-node-edit'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_SELECT = '用户痛点'; MW_CONCEPT_EDIT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-node-edit.txt' } },
    @{ Name = 'cm-link-create'; Dir = 'shots/golden/cm-link-create'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK = '用户痛点|风险与依赖'; MW_WINDOW_SIZE = '1280x800' } },
    # 打磨轮 3（C9）：同一条新命题线，拍在**过渡中途**。`MW_MOTION_T` 只给验收用——
    # 它把 `blend_scenes` 的混合系数钉死在 0.5，于是「淡入到一半」的那一帧停在屏幕上，
    # 基线才锁得住（不设它时离屏一律 `reduce_motion`，过渡一帧都不起）。
    # 与上面 `cm-link-create` 是同一份文档、同一帧布局，差别只有线的透明度：
    # 这张的线身取 0.5 透明度、选中强调 3px × 0.5，稳态那张是 1.0 / 1.0。
    @{ Name = 'cm-link-fade-mid'; Dir = 'shots/golden/cm-link-fade-mid'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK = '用户痛点|风险与依赖'; MW_MOTION_T = '0.5'; MW_WINDOW_SIZE = '1280x800' } },
@{ Name = 'cm-link-blank'; Dir = 'shots/golden/cm-link-blank'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK_BLANK = '用户痛点|240|40'; MW_WINDOW_SIZE = '1280x800' } },
@{ Name = 'cm-link-drag'; Dir = 'shots/golden/cm-link-drag'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK_DRAG = '用户痛点|240|40'; MW_WINDOW_SIZE = '1280x800' } },
@{ Name = 'cm-relink-blank'; Dir = 'shots/golden/cm-relink-blank'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_RELINK_BLANK = '带来|200|140'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-label-ime'; Dir = 'shots/golden/cm-label-ime'; Ppp = '1.0'; Warmup = 18; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_REL_EDIT = '带来'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-label-ime.txt' } },
    # 打磨轮 40（C16）：短语留空的两种端到端结果。
    #   `cm-phrase-clear`：既有「带来」→ 全选删除 → Enter。命令必须**不发**
    #     （轨迹 `rel-label-reject reason=empty`），短语留在画布上，底部提示说明原因。
    #   `cm-phrase-blank`：新建「用户痛点 → 风险与依赖」→ Enter 留空。整条线撤掉
    #     （`reason=blank_new`），之后的 `scene_rel` 不再出现 `w=0` 的线团；概念留下。
    #   `cm-outline-via-clear`：大纲同一条口径（`outline_concept_via_reject`）。
    @{ Name = 'cm-phrase-clear'; Dir = 'shots/golden/cm-phrase-clear'; Ppp = '1.0'; Warmup = 26; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_REL_EDIT = '带来'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-phrase-clear.txt' } },
    @{ Name = 'cm-phrase-blank'; Dir = 'shots/golden/cm-phrase-blank'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK = '用户痛点|风险与依赖'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-phrase-blank.txt' } },
    @{ Name = 'cm-outline-via-clear'; Dir = 'shots/golden/cm-outline-via-clear'; Ppp = '1.0'; Warmup = 26; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-via-clear.txt' } },
    # CM-09 新增三张「大纲改结构」：Tab 建边 / Shift+Delete 断边 / Shift+F2 改短语。
    @{ Name = 'cm-outline-link'; Dir = 'shots/golden/cm-outline-link'; Ppp = '1.0'; Warmup = 44; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-link.txt' } },
    @{ Name = 'cm-outline-unlink'; Dir = 'shots/golden/cm-outline-unlink'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '增长策略'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-unlink.txt' } },
    # 2026-10-26 夜重开：短语框以前在离屏里立不住（夹具每帧抢焦点），
    # 改成 `MW_OUTLINE_SELECT` 只武装一次 + `RowKey::Delete` 在短语框立着时不执行之后，
    # 这个场景能稳定停在「短语框里带文字」的状态。
    @{ Name = 'cm-outline-via'; Dir = 'shots/golden/cm-outline-via'; Ppp = '1.0'; Warmup = 26; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-via.txt' } },
    # 打磨轮 56（C20 概念图大纲行菜单）：行尾 `⋯` 与行右键共用一张「命题网」菜单
    #   （7 条动作 + 2 条分隔线），没有新建子主题/同级、折叠、聚焦等树语义。
    #   `cm-outline-menu` 停在菜单展开；`-cancel` 按 Esc 后只有焦点/选中差，
    #   轨迹里没有 `outline_row_action`（未落地任何命令）。
    @{ Name = 'cm-outline-menu'; Dir = 'shots/golden/cm-outline-menu'; Ppp = '1.0'; Warmup = 3; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-menu.txt' } },
    @{ Name = 'cm-outline-menu-cancel'; Dir = 'shots/golden/cm-outline-menu-cancel'; Ppp = '1.0'; Warmup = 3; Env = @{ MW_CONCEPT_OUTLINE = '1'; MW_OUTLINE_SELECT = '用户痛点'; MW_WINDOW_SIZE = '1280x800'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-outline-menu-cancel.txt' } }
    # CM-14（2026-11-01）：概念图「连接」体系五张。夹具都走真实函数
    # （`begin_relationship_action` / `reverse_selected_relationship` / `start_rel_edit`），
    # 所以锁的是真状态。**这五张基线尚未生成**：本机没有 wgpu 适配器，
    # 第一次在有 GPU 的机器上要带 `-Update` 跑一遍，并按 `shots/x100/CM-14/report.md` §3 核对画面。
    #   `cm-link-menu`：概念节点菜单——「新建概念并连接 / 新建独立概念 / 连接到…」三行。
    #   `cm-link-mode`：选中一颗概念后的连线态（源强调环 + 提示条「点击另一个概念建立连接」）。
    #   `cm-link-two`：选中两颗后直连，新命题落地且立刻是短语编辑态（空框「＋ 链接词」）。
    #   `cm-rel-reverse`：「支撑」那条边反向——箭头换端、短语仍在原处。
    #   `cm-phrase-suggest`：短语框输入「支」时下拉列出本图已有的「支撑」。
    @{ Name = 'cm-link-menu'; Dir = 'shots/golden/cm-link-menu'; Ppp = '1.0'; Warmup = 6; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONTEXT_DEMO = 'concept:用户痛点'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-link-mode'; Dir = 'shots/golden/cm-link-mode'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK_MODE = '用户痛点'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-link-two'; Dir = 'shots/golden/cm-link-two'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_LINK_TWO = '用户痛点|技术方案'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-rel-reverse'; Dir = 'shots/golden/cm-rel-reverse'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_REVERSE = '支撑'; MW_WINDOW_SIZE = '1280x800' } },
    @{ Name = 'cm-phrase-suggest'; Dir = 'shots/golden/cm-phrase-suggest'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_SUGGEST = '带来|支'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-16（2026-11-01）：线型 / 线宽 / 命题注释。夹具走三条真命令，所以这张图同时锁住
    # 数据落盘、`Scene` 的两个新字段、以及检查器里的「线型 / 线宽 / 注释」三组控件。
    # 画面：`带来` 那条线变成 2.5 粗的虚线（6/4 节奏），检查器显示「虚线 / 粗 / 2026Q3 的数据」。
    @{ Name = 'cm-rel-style'; Dir = 'shots/golden/cm-rel-style'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_REL_STYLE = '带来'; MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-17（2026-11-01）：多选命题。两条线同时 3px 强调，检查器切「2 条命题」批量页
    # （箭头四格 / 线型 / 线宽 / 删除 2 条）。夹具走 `toggle`，锁的就是「加选」这条真实路径。
    @{ Name = 'cm-rel-multi'; Dir = 'shots/golden/cm-rel-multi'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_MULTI_REL = '支撑|带来'; MW_INSPECTOR = '1'; MW_WINDOW_SIZE = '1280x800' } },
    # CM-18（2026-11-01）：概念分组。框由成员包围盒 + 24 内距派生，圆角 12、1.5px、6/4 虚线、
    # 6% 淡底，名字画在框的上沿外侧。夹具走「选中两颗 → 创建分组」这条真实路径。
    @{ Name = 'cm-group'; Dir = 'shots/golden/cm-group'; Ppp = '1.0'; Warmup = 8; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_GROUP = '用户痛点|增长策略'; MW_WINDOW_SIZE = '1280x800' } },
    # BUG-10（2026-11-02 排查收口）：落点重叠的告警边。
    # 单颗概念拖拽会走 `free_concept_spot` 落点避让，永远不重叠——脚本先点「用户痛点」
    # 再 Shift 加选「增长策略」，两颗一起拖（`solo=false`）才会真的压上别人。
    # 告警的时限读 `Canvas::now`（金样是 M16 帧计数虚拟时钟）；**截帧必然晚于脚本结束**：
    # `app_methods_1.rs` 把 warmup 提到 `脚本帧数 + 4`，而 harness 的帧计数从脚本跑完才起算，
    # 实测等到截帧时虚拟时钟已走到 ~1.18s，0.25s 的真窗口活不到那一刻。所以这张用验收旋钮
    # `MW_OVERLAP_FLASH_MS=2000` 把窗口拉长（真机不设它，仍是 0.25s；单测钉住 0.25s 边界）。
    @{ Name = 'cm-overlap-flash'; Dir = 'shots/golden/cm-overlap-flash'; Ppp = '1.0'; Warmup = 32; Env = @{ MW_CONCEPT_DEMO = '1'; MW_WINDOW_SIZE = '1280x800'; MW_OVERLAP_FLASH_MS = '2000'; MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-overlap-flash.txt' } },
    # 第 51 轮：注解（概要）的选中环与跨度手柄。这两处覆盖层在第 50 轮之前画在合成纹理贴图**之前**，
    # 在界面上从来没显示过——这张是它们的第一张、也是唯一一张像素证据。
    # 选 `tiny-cn` 思维导图夹具而不是概念图示例：概念图模板是平铺的（根节点不挂子级），
    # 而概要/边界要求「目标是父节点的直接子节点」，在平铺图上根本建不出来。
    @{ Name = 'mm-summary-select'; Dir = 'shots/golden/mm-summary-select'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_SUMMARY_SELECT = '两条主线先合并'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 52 轮：**边界**的选中环。与概要共用同一段绘制代码、几何不同（边界圈住一片区域并带淡色底），
    # 不单独拍一张就没有证据。同一份 `tiny-cn` 夹具，只换 `MW_BOUNDARY_SELECT`。
    @{ Name = 'mm-boundary-select'; Dir = 'shots/golden/mm-boundary-select'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_BOUNDARY_SELECT = '1'; MW_WINDOW_SIZE = '1280x800' } }
    # 打磨轮 37（M17 悬停反馈）：注解也「指得到」——不选中，只把指针停在上面。
    #   `mm-summary-hover`：停在概要主题盒 (176,481)，主题盒外扩 2px 描圈 + 花括号/连接线加亮，
    #     与 `mm-summary-select` 差 1 484 px（bbox 只在概要本身）。
    #   `mm-boundary-hover`：停在边界左边线 (1123,482)，圆角圈外扩 2px 描一圈。
    #   两处坐标都按落定相机用 `scene_summary topic=` / `scene_boundary rect=` 量出；
    #   旧注释里的 (125,428)/(1073,675) 是落定前的估计，已作废。
    @{ Name = 'mm-summary-hover'; Dir = 'shots/golden/mm-summary-hover'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-summary-hover.txt'; MW_WINDOW_SIZE = '1280x800' } }
    @{ Name = 'mm-boundary-hover'; Dir = 'shots/golden/mm-boundary-hover'; Ppp = '1.0'; Warmup = 20; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-boundary-hover.txt'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 53 轮：**跨度拖拽中**的预览框（`paint_span_preview` 读 `sd.preview`，同样曾被合成纹理盖掉）。
    # 与 `mm-summary-select` 构成「只差一个拖拽状态位」的对照：两张的强调绿像素数完全相同（852），差异只在预览带。
    @{ Name = 'mm-span-drag'; Dir = 'shots/golden/mm-span-drag'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_SPAN_DRAG = '两条主线先合并'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 54 轮：**概要 + 边界同时选中**（additive 选择唯一能被看见的形态）。
    # 强调绿 4 381 px ≈ 概要单选的 852 + 边界单选的 3 983 的并集，两圈环都在。
    @{ Name = 'mm-annot-multi'; Dir = 'shots/golden/mm-annot-multi'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_ANNOT_MULTI = '两条主线先合并'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 56 轮：多选后**取消**边界，应当不留残影。判据不是「新图好看」而是三条数字：
    # 强调绿 838（与 `mm-summary-select` 的 838 完全相同）；与 `mm-annot-multi` 差 3 284 强像素、只在边界环带；
    # 与 `mm-summary-select` 0 强像素（60 654 弱差是边界自身的淡色底）。
    @{ Name = 'mm-annot-multi-off'; Dir = 'shots/golden/mm-annot-multi-off'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_ANNOT_MULTI_OFF = '两条主线先合并'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 57 轮：跨度拖拽**松手提交**的终态。判据：文档目标 2→3（`span_commit before=2 after=3 changed=true`）、
    # 撤销一次回到 2（`span_commit_undo back=2`）、重做回到 3；画面与 `mm-summary-select` 差 7 198 px，
    # 且**全部**落在 y440..610 这一带（上方 0、下方 0）⇒ 版面没重排，只有花括号变长。
    @{ Name = 'mm-span-commit'; Dir = 'shots/golden/mm-span-commit'; Ppp = '1.0'; Warmup = 10; Env = @{ MW_OPEN = 'tests/fixtures/x100/tiny-cn.zmind'; MW_SPAN_COMMIT = '两条主线先合并'; MW_WINDOW_SIZE = '1280x800' } }
    # 第 55 轮：标签弹层的**封顶 + 滚动**（docs/30 §5.11）。120 个标签 → 66 行，视口封在 639、
    # 总高 653、底边 790（窗口 800，留 8 边距）；弹层不越过窗口，内容能滚。
    @{ Name = 'cm-outline-label-many'; Dir = 'shots/golden/cm-outline-label-many'; Ppp = '1.0'; Warmup = 16; Env = @{ MW_CONCEPT_DEMO = '1'; MW_OUTLINE = '1'; MW_OUTLINE_LABELS = '120'; MW_OUTLINE_LABEL_POPUP = '1'; MW_WINDOW_SIZE = '1280x800' } }
)

function Run-GoldenScene($scene) {
    # CM-14：五张新场景的基线还没生成（本机无 wgpu 适配器，见 `shots/x100/CM-14/report.md` §4.3）。
    # 缺基线时**明确跳过并记一行**——不静默、也不把「没拍」算成「通过」：
    # 末尾会把跳过数打出来，跑一次 `-Update` 即补齐。
    $baseline = Join-Path $root ($scene.Dir + '\shell-ppp' + ('{0:0.00}' -f [double]$scene.Ppp) + '.png')
    if (-not $Update -and -not (Test-Path -LiteralPath $baseline)) {
        Say "SKIP golden $($scene.Name)：基线不存在（$baseline）"
        $script:goldenSkipped += 1
        return 0
    }
    $app = Join-Path $root "target/ci-appdata/cm-gate-$($scene.Name)"
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
        # 脚本帧数必须被 warmup 吃满。拍在脚本中途的图是**确定**的（每帧都一样），
        # 所以它不会飘，只会悄悄锁错画面——比红更糟。cargo golden 自己不管这件事，这里管。
        $scriptFrames = 0
        $warmupSeen = 0
        foreach ($line in $out) {
            $t = [string]$line
            if ($t -match '输入脚本 (\S+) · (\d+) 帧') { $scriptFrames = [int]$Matches[2] }
            if ($t -match 'warmup (\d+) 帧') { $warmupSeen = [int]$Matches[1] }
        }
        if ($scriptFrames -gt 0) {
            if ($warmupSeen -lt $scriptFrames) {
                Say "FAIL script frames: warmup $warmupSeen < 脚本 $scriptFrames 帧（拍到脚本中途）"
                return 1
            }
            Say "PASS script frames: $scriptFrames <= warmup $warmupSeen"
        }
        foreach ($line in $out) { Say ([string]$line) }
        return [int]$LASTEXITCODE
    } finally {
        foreach ($k in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process')
        }
    }
}

Say "G-CM-2 machine gate (CM-06..CM-09)"
if ($Update) { Say 'MODE: re-shoot baselines (--update) — 外观有意改动，旧哈希需登记进 manifest' }
Say "root=$root"
Say "started=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$null = Run-Step 'cargo fmt --check' { cargo fmt --all -- --check }

if (-not $SkipClippy) {
    $null = Run-Step 'mw-shell clippy: no new warnings' {
        $output = & cargo clippy -p mw-shell --all-targets 2>&1
        $code = $LASTEXITCODE
        $output | Set-Content -LiteralPath (Join-Path $logDir 'stage-cm-clippy.log') -Encoding UTF8
        # 「新增 0」的口径：只看本轨动过的文件。`cargo clippy -p mw-shell` 会连带编
        # mw-core / mw-render / mw-io 并把它们的历史告警一起打出来（27 条，`docs/24`
        # 已登记），拿「一条都不许有」去卡会把既有告警当成新债——那不是更严，是读错。
        $touchedFiles = @(
            'crates\\mw-shell\\src\\canvas\.rs',
            'crates\\mw-shell\\src\\canvas\\',
            'crates\\mw-shell\\src\\app_methods_1\.rs',
            'crates\\mw-shell\\src\\app_methods_2\.rs',
            'crates\\mw-shell\\src\\main\.rs',
            'crates\\mw-shell\\src\\panels\\outline\.rs'
        )
        $touched = @($output | Where-Object {
            $line = [string]$_
            if ($line -notmatch '^\s*--> ') { return $false }
            foreach ($t in $touchedFiles) { if ($line -match $t) { return $true } }
            return $false
        })
        if ($touched.Count -gt 0) {
            $touched | ForEach-Object { Say "CM-warning $_" }
            throw 'new clippy warning'
        }
        if ($code -ne 0) { throw "clippy failed: $code" }
    }
}

# `cm-roundtrip`（docs/30 §5.10）：`.zmind` 往返逐字段相等，且版本未升。
$null = Run-Step 'concept map .zmind roundtrip' {
    $output = & cargo test -p mw-format concept_map_roundtrips_positions_and_relationships -- --test-threads=1 2>&1
    $code = $LASTEXITCODE
    foreach ($line in ($output | Select-Object -Last 6)) { Say ([string]$line) }
    if ($code -ne 0) { throw "roundtrip failed: $code" }
}

if (-not $SkipFullTests) {
    Say ""
    Say '==> full serial workspace tests'
    $output = & cargo test --workspace -- --test-threads=1 2>&1
    $code = $LASTEXITCODE
    $output | Set-Content -LiteralPath (Join-Path $logDir 'stage-cm-tests.log') -Encoding UTF8
    $counts = Get-TestCounts $output
    foreach ($line in ($output | Select-Object -Last 12)) { Say ([string]$line) }
    Say "tests passed=$($counts.Passed) failed=$($counts.Failed) exit=$code"
    if ($code -ne 0 -or $counts.Failed -ne 0 -or $counts.Passed -lt 813) {
        $failures.Add("full tests (passed $($counts.Passed), failed $($counts.Failed), exit $code)")
        Say 'FAIL full serial workspace tests'
    } else {
        Say 'PASS full serial workspace tests'
    }
}

if (-not $SkipGolden) {
    cargo build --release -p mw-shell | Out-Host
    if ($LASTEXITCODE -ne 0) { $failures.Add('release build'); Say 'FAIL release build' }
    foreach ($pass in 1..2) {
        $null = Run-Step "concept golden pass $pass ($($scenes.Count) comparisons)" {
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
if ($goldenSkipped -gt 0) {
    # 明确写出来：跳过不是通过。缺基线的场景要先用 `-Update` 生成（两遍各计一次）。
    Say "WARN golden skipped=$goldenSkipped 次（缺基线；两遍合计。先跑 -Update 生成，再按 shots/x100/CM-14/report.md §3 核对画面）"
}
if ($failures.Count -gt 0) {
    Say "MACHINE GATE FAIL: $($failures -join '; ')"
    $code = 1
} else {
    Say 'MACHINE GATE PASS; HUMAN G-CM-2 REVIEW IS STILL REQUIRED'
    $code = 0
}
Say "finished=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$logPath = Join-Path $logDir 'stage-cm-gate.log'
$log | Set-Content -LiteralPath $logPath -Encoding UTF8
Write-Host "log=$logPath"
exit $code
