//! 语义化命令：前端只发命令，core 执行并生成可逆变更集。

use serde::Deserialize;

use crate::model::{ArrowHead, LayoutSettings, Node, StyleOverride};
use crate::note::NoteCard;

#[derive(Deserialize, Clone, Debug)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// 在父节点下新建一个节点（`index` 为 None 时追加到末尾）。
    ///
    /// **概念图里一律被拒**（`Invalid("概念图里概念不能挂成树节点")`）：这条命令只带
    /// `parent`、不带位置，概念图里没有「父节点」这一层。概念图新建概念走
    /// [`Command::AddFloatingTopic`]（自由摆放）或 [`Command::AddLinkedConcept`]（建概念 + 连线）。
    AddNode {
        parent: String,
        index: Option<usize>,
        text: String,
        style: Option<StyleOverride>,
    },
    /// 插入一整棵（或并列多棵）子树（粘贴/模板）。
    /// `nodes` 的父子关系必须自洽；`parentId == parent` 的顶层节点可有 1..N 个，
    /// 它们会按顺序插入到 `index` 处。
    ///
    /// **概念图里一律被拒**（子树就是 `parent_id = Some(..)`）；概念图粘贴概念走
    /// [`Command::InsertConcepts`]。
    InsertSubtree {
        parent: String,
        index: Option<usize>,
        nodes: Vec<Node>,
    },
    /// 删除若干节点及其子树（根节点受保护）。
    DeleteNodes {
        ids: Vec<String>,
    },
    SetText {
        id: String,
        text: String,
    },
    SetNote {
        id: String,
        note: Option<String>,
    },
    SetLink {
        id: String,
        link: Option<String>,
    },
    /// 设置图标标记（优先级/任务状态/星标等）；`icon = None` 表示清除。
    SetIcon {
        ids: Vec<String>,
        icon: Option<String>,
    },
    /// 设置样式覆盖；`style = None` 表示清除覆盖。
    SetStyle {
        ids: Vec<String>,
        style: Option<StyleOverride>,
    },
    SetCollapsed {
        id: String,
        collapsed: bool,
    },
    /// 全局折叠/展开（根节点保持展开）。
    SetCollapsedAll {
        collapsed: bool,
    },
    /// 把节点挂到 `new_parent` 之下（树结构）。
    ///
    /// **概念图里这条命令一律被拒**（`Invalid("概念图里概念不能挂成树节点")`）：概念没有
    /// 父子关系，概念之间的连接走 `relationships`；落在这里会把 `parent_id` 写成
    /// `Some(..)` 并且把 `pos` 抹掉，等于把概念静默降级成树节点。
    /// 概念图里要挪位置用 [`Command::MoveConcept`]。
    MoveNode {
        id: String,
        new_parent: String,
        index: Option<usize>,
    },
    SetLayout {
        layout: LayoutSettings,
    },
    /// 概念图的聚焦问题：这张图要回答的那句话。`None` = 清空。
    ///
    /// 单独一条命令（而不是复用 `SetLayout`）是为了撤销文案说人话——
    /// 「更换布局」出现在撤销菜单里会让人以为整张图的布局被动了。
    SetFocusQuestion {
        text: Option<String>,
    },
    SetTheme {
        theme: String,
    },
    /// 自定义强调色（`#rrggbb`）；`None` = 回到主题自带的强调色。只取色相，饱和度与明度
    /// 由各主题保住，见 `Theme::with_accent`。
    SetThemeAccent {
        accent: Option<String>,
    },
    /// 文档级主题微调（画布底色 / 中心卡 / 一级卡）；`None` = 清掉全部微调。
    SetThemeOverrides {
        overrides: Option<crate::model::ThemeOverrides>,
    },
    // ---------------- 高级元素（XMind 附件类） ----------------
    /// 添加关系线：从 `from` 指向 `to`（可带标签文字）。
    ///
    /// 概念图里**同一对概念之间可以有多条边**（两条边 = 两条不同的命题，如
    /// 「A 影响 B」+「A 包含 B」）；思维导图里仍只留一条，理由见 `apply_inner`
    /// 该分支的注释（平行边在 `scene.rs` 里会完全重合）。自连一律禁止。
    AddRelationship {
        from: String,
        to: String,
        label: Option<String>,
    },
    /// 修改关系线标签；`label = None` 表示清除。
    ///
    /// 概念图里**不许清除**（`Invalid("概念图的连接短语必填")`）：没有短语的线不构成命题。
    /// 但「新建时留空」是合法的——`AddRelationship` / [`Command::AddLinkedConcept`] 建线时
    /// 落 `None`，画布立刻立起短语框让用户当场写；堵住的只有「把已有短语清空」。
    SetRelationshipLabel {
        from: String,
        id: String,
        label: Option<String>,
    },
    /// 改关系线的颜色；`None` 跟随主题。
    SetRelationshipColor {
        from: String,
        id: String,
        color: Option<String>,
    },
    /// 改关系线的端点箭头方向（CM-10 ⑦）。一条撤销记录。
    SetRelationshipHead {
        from: String,
        id: String,
        head: ArrowHead,
    },
    /// 关系线线型：实线 / 虚线（`docs/30` §5.4.3，CM-16）。
    ///
    /// 虚线用来表达「暂定 / 推测」这类**弱命题**——CmapTools 里同样靠线型区分
    /// 确定的因果与待验证的猜测，而不必再造一个新短语。
    SetRelationshipDashed {
        from: String,
        id: String,
        dashed: bool,
    },
    /// 关系线线宽（css px，0.5–6.0）；`None` = 跟随主题（1.5，屏幕空间恒定）。
    SetRelationshipWidth {
        from: String,
        id: String,
        width: Option<f32>,
    },
    /// 概念分组（CM-18，`docs/32` §13）：把若干概念框成一组。
    ///
    /// 成员 < 2 拒绝；成员已在别的组里 → 先从旧组摘掉（一个概念只属于一个组）。
    CreateGroup {
        members: Vec<String>,
        label: String,
    },
    /// 解组：只删组框，概念一个不删。
    DeleteGroup {
        id: String,
    },
    /// 重命名分组（空串回落「分组」）。
    SetGroupLabel {
        id: String,
        label: String,
    },
    /// 改分组框色；`None` = 跟随主题。
    SetGroupColor {
        id: String,
        color: Option<String>,
    },
    /// 整组平移（成员一起动，一条撤销）。
    MoveGroup {
        id: String,
        dx: f32,
        dy: f32,
    },

    /// 关系线注释：给这条命题挂一段解释。`None` / 空串 = 清除。
    ///
    /// 与链接短语分开存：短语是谓词（「导致」），注释是解释（「2026Q3 的数据」），
    /// 前者进检索标题、后者只在检查器里读。
    SetRelationshipNote {
        from: String,
        id: String,
        note: Option<String>,
    },
    /// 改关系线的目标节点（拖端点换目标）；`to` 必须是另一条已存在的节点。
    SetRelationshipTarget {
        from: String,
        id: String,
        to: String,
    },
    /// 反向连接：把这条命题的两端对调（`docs/30` §5.4.2，CM-14）。
    ///
    /// 边从 `from` 名下挪到原目标名下，`to` 改成 `from`；短语、线色、边 id 原样保留，
    /// 箭头**镜像**（`→` ↔ `←`，`无`/`双向` 不变）。一条撤销记录。
    ///
    /// 为什么不是「删掉再建一条」：删+建是两条撤销记录，且新边的 id 变了——
    /// 用户按一次 `Ctrl+Z` 只会退回一半，画布上留下一条方向仍然错的线。
    ///
    /// 思维导图里也成立，但仍守「一对节点之间只有一条边」：反向后目标端已有指向
    /// `from` 的边时拒绝（概念图允许多条平行命题，不受此限）。
    ReverseRelationship {
        from: String,
        id: String,
    },
    DeleteRelationship {
        from: String,
        id: String,
    },
    /// 在父节点上添加边界，圈住 `targets`（必须是其直接子节点）。
    AddBoundary {
        parent: String,
        targets: Vec<String>,
        color: Option<String>,
    },
    DeleteBoundary {
        parent: String,
        id: String,
    },
    /// 改边界圈住的兄弟节点（跨度）。目标必须仍是该父节点的直接子节点，至少一个。
    SetBoundaryTargets {
        parent: String,
        id: String,
        targets: Vec<String>,
    },
    /// 改边界颜色（`#rrggbb`）；`None` = 回到主题默认。
    SetBoundaryColor {
        parent: String,
        id: String,
        color: Option<String>,
    },
    /// 在父节点上添加概要（覆盖 `targets` 直接子节点，带摘要文字）。
    AddSummary {
        parent: String,
        targets: Vec<String>,
        text: String,
    },
    SetSummaryText {
        parent: String,
        id: String,
        text: String,
    },
    /// 改概要圈住的兄弟节点（跨度）。目标必须仍是该父节点的直接子节点，至少一个。
    SetSummaryTargets {
        parent: String,
        id: String,
        targets: Vec<String>,
    },
    DeleteSummary {
        parent: String,
        id: String,
    },
    /// 给若干节点添加/移除标签。
    AddLabel {
        ids: Vec<String>,
        label: String,
    },
    RemoveLabel {
        ids: Vec<String>,
        label: String,
    },
    /// 新建自由主题（XMind 浮动主题），世界坐标 (x, y) 由前端给出。
    AddFloatingTopic {
        text: String,
        x: f32,
        y: f32,
    },
    /// 在概念图里插入一批**独立浮动概念**（复制 / 粘贴 / Ctrl+D / Ctrl+拖拽）。
    ///
    /// 与 [`Command::InsertSubtree`] 的差别是结构语义：这里没有父级，所有节点都必须
    /// `parent_id = None`、带 `pos`，关系线只表达命题网；一条命令一次撤销。
    /// 只在 `kind == "conceptMap"` 的文档里成立，不改 `.zmind` 字段。
    InsertConcepts {
        nodes: Vec<Node>,
    },
    /// 移动自由主题（拖拽摆放）。
    SetNodePos {
        id: String,
        x: f32,
        y: f32,
    },
    /// 摆放概念图里的概念。与 [`Command::SetNodePos`] 的差别只有一条：
    /// **允许移动 `root_id` 指向的那个概念**（概念图的主概念也是可拖的）。
    /// 只在 `kind == "conceptMap"` 的文档里成立，别的文档一律拒绝。
    MoveConcept {
        id: String,
        x: f32,
        y: f32,
    },
    /// 钉住 / 取消钉住概念图里的一个概念（`docs/30` §5.5 的「手工位置受保护」）。
    ///
    /// 钉住的概念在「整理布局」「整理选中」「力导向」里**原样不动**
    /// （`concept_layout::arrange_scoped` / `flow` 的 `fixed` 槽位）。
    /// 只在 `kind == "conceptMap"` 的文档里成立，别的文档一律拒绝——
    /// 和 [`Command::MoveConcept`] 同一道门：钉住写的是 `Node.pinned`，
    /// 对树节点没有意义，静默接受只会造出读不出来的脏状态。
    SetConceptPinned {
        id: String,
        pinned: bool,
    },
    /// 给节点挂一张已有卡片；`card = None` 表示解除链接（卡片保留）。
    SetNodeCard {
        node: String,
        card: Option<String>,
    },
    /// 新建一张卡片并挂到节点上（一条命令、一次撤销）。
    AttachNewCard {
        node: String,
        card: NoteCard,
    },
    /// 改卡片正文（Markdown）。
    SetCardBody {
        id: String,
        body: String,
    },
    /// 改卡片标题。
    SetCardTitle {
        id: String,
        title: String,
    },
    /// 改卡片标签（会归一化：trim、去重、限量）。
    SetCardTags {
        id: String,
        tags: Vec<String>,
    },
    /// 设置节点任务状态；`None` 清除。
    SetTask {
        id: String,
        task: Option<crate::model::TaskState>,
    },
    /// 设置节点图片占位（P41；实际加载 P53）。
    SetImage {
        id: String,
        image: Option<String>,
    },
    /// 全文档重命名一个标签（大小写无关）。
    RenameTag {
        from: String,
        to: String,
    },
    /// 把树内节点分离为自由主题（拖到空白处）。
    DetachNode {
        id: String,
        x: f32,
        y: f32,
    },
    /// 把选中的树内节点提为**中心主题**（M31「设为根主题」）。
    ///
    /// 旧根连同它剩下的整棵树挂到新根之下（追加在末尾），新根从原父级摘除；
    /// 新根自己的子树保持不动。一条命令、一次撤销（`ChangeSet::with_root` 记下前后根）。
    ///
    /// **只在思维导图里合法**：概念图的 `root_id` 只是「第一个概念」，
    /// 没有中心主题的树语义（`Invalid("概念图里没有中心主题")`）。
    SetRoot {
        id: String,
    },
    /// 新建一个浮动概念并**立刻**连一条边到 `from`（CM-10 ①：拖到空白处松手）。
    ///
    /// 为什么不是「`AddFloatingTopic` + `AddRelationship`」两步：那是两条撤销记录，
    /// 用户按一次 Ctrl+Z 会剩下一个没有边的孤儿概念。对标 CmapTools 应当是**一步回去**。
    ///
    /// **只在概念图里合法**（`Invalid("只有概念图可以新建概念并连线")`）：它写出的是
    /// `parent_id = None` + `pos` 的浮点节点，思维导图的结构里没有这种节点。
    AddLinkedConcept {
        from: String,
        text: String,
        x: f32,
        y: f32,
    },
    /// 在空白处新建一个概念，并把 `from` 上的关系线 `id` 改指到它（CM-10 ①）的姊妹手势。
    ///
    /// 与 `AddLinkedConcept` 同一个理由：如果「建概念」和「改目标」分两步下发，
    /// 撤销一次会剩一个没人连的新概念。
    ///
    /// 同样**只在概念图里合法**。
    RetargetLinkToNewConcept {
        from: String,
        id: String,
        text: String,
        x: f32,
        y: f32,
    },
    /// 批量命令：单条撤销记录，失败整体回滚。
    Batch {
        commands: Vec<Command>,
    },
}

/// 命令的中文标签（撤销提示等 UI 文案）。
pub fn label(cmd: &Command) -> &'static str {
    match cmd {
        Command::AddNode { .. } => "新建节点",
        Command::InsertSubtree { .. } => "粘贴节点",
        Command::DeleteNodes { .. } => "删除节点",
        Command::SetText { .. } => "修改文字",
        Command::SetNote { .. } => "修改备注",
        Command::SetLink { .. } => "修改链接",
        Command::SetIcon { .. } => "设置图标标记",
        Command::SetStyle { .. } => "修改样式",
        Command::SetCollapsed { .. } => "折叠/展开",
        Command::SetCollapsedAll { .. } => "批量折叠",
        Command::MoveNode { .. } => "移动节点",
        Command::SetLayout { .. } => "更换布局",
        Command::SetFocusQuestion { .. } => "编辑聚焦问题",
        Command::SetTheme { .. } => "更换主题",
        Command::SetThemeAccent { .. } => "更换强调色",
        Command::SetThemeOverrides { .. } => "微调主题配色",
        Command::AddRelationship { .. } => "添加关系线",
        Command::SetRelationshipLabel { .. } => "修改关系线",
        Command::SetRelationshipColor { .. } => "修改关系线颜色",
        Command::SetRelationshipHead { .. } => "修改关系线方向",
        Command::SetRelationshipDashed { dashed, .. } => {
            if *dashed {
                "改为虚线"
            } else {
                "改为实线"
            }
        }
        Command::SetRelationshipWidth { width, .. } => {
            if width.is_some() {
                "修改线宽"
            } else {
                "恢复默认线宽"
            }
        }
        Command::CreateGroup { .. } => "创建分组",
        Command::DeleteGroup { .. } => "解组",
        Command::SetGroupLabel { .. } => "重命名分组",
        Command::SetGroupColor { .. } => "修改分组颜色",
        Command::MoveGroup { .. } => "移动分组",
        Command::SetRelationshipNote { note, .. } => {
            if note.is_some() {
                "编辑命题注释"
            } else {
                "清除命题注释"
            }
        }
        Command::SetRelationshipTarget { .. } => "调整关系线",
        Command::ReverseRelationship { .. } => "反向连接",
        Command::DeleteRelationship { .. } => "删除关系线",
        Command::AddBoundary { .. } => "添加边界",
        Command::DeleteBoundary { .. } => "删除边界",
        Command::SetBoundaryColor { .. } => "修改边界颜色",
        Command::SetBoundaryTargets { .. } => "调整边界跨度",
        Command::AddSummary { .. } => "添加概要",
        Command::SetSummaryText { .. } => "修改概要",
        Command::SetSummaryTargets { .. } => "调整概要跨度",
        Command::DeleteSummary { .. } => "删除概要",
        Command::AddLabel { .. } => "添加标签",
        Command::RemoveLabel { .. } => "移除标签",
        Command::AddFloatingTopic { .. } => "新建自由主题",
        Command::InsertConcepts { .. } => "粘贴概念",
        Command::AddLinkedConcept { .. } => "新建概念并连线",
        Command::RetargetLinkToNewConcept { .. } => "改线到新概念",
        Command::SetNodePos { .. } => "移动自由主题",
        Command::MoveConcept { .. } => "摆放概念",
        // 按值分两个标签：撤销提示里「钉住概念」和「取消钉住」是两回事，
        // 用户要能从栈里读出刚才做的是哪一种（与 `SetNodeCard` 的先例同款）。
        Command::SetConceptPinned { pinned, .. } => {
            if *pinned {
                "钉住概念"
            } else {
                "取消钉住"
            }
        }
        Command::SetNodeCard { card, .. } => {
            if card.is_some() {
                "关联卡片"
            } else {
                "解除卡片"
            }
        }
        Command::AttachNewCard { .. } => "新建卡片笔记",
        Command::SetCardBody { .. } => "编辑卡片正文",
        Command::SetCardTitle { .. } => "修改卡片标题",
        Command::SetCardTags { .. } => "修改卡片标签",
        Command::SetTask { .. } => "修改任务状态",
        Command::SetImage { .. } => "修改节点图片",
        Command::RenameTag { .. } => "重命名标签",
        Command::DetachNode { .. } => "分离为自由主题",
        Command::SetRoot { .. } => "设为根主题",
        Command::Batch { commands } => commands.first().map(label).unwrap_or("批量操作"),
    }
}
