use super::*;

/// 概念图里新建的命题还没写短语时的占位（`docs/33` §2 B1a）。
///
/// 线**先落盘**、短语留这个占位，用户事后再补——改前是「留空就把线撤掉」，
/// 用户的体感是「连不上」：拖完松手、没来得及打字点到别处，线就没了。
/// 现在只有显式 `Esc` 才撤线。
pub(crate) const CONCEPT_UNNAMED_LINK: &str = "未命名";

/// 短语编辑框按下 Enter / 失焦时的判定（打磨轮 40 · C16 → `docs/33` §2 B1a）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PhraseCommit {
    /// 一字未动：不发命令（不压撤销栈、不把文档标脏）。
    Unchanged,
    /// 概念图里**既有命题**被清空：不提交、保留原短语并提示。
    RejectEmpty,
    /// 正常提交；`None` 只在思维导图里表示清空短语（概念图模型层会拒绝）。
    Set(Option<String>),
}

/// 判定短语编辑框的提交结果。
///
/// 模型层对「概念图 + 空短语」的 `SetRelationshipLabel` 一律拒绝
/// （`Invalid("概念图的连接短语必填")`）。壳层先判一次，才能把「为什么没改成」
/// 说出来，而不是发一条必然失败的命令、再让编辑器静默关掉。
///
/// **新建的线留空不再撤线**（`docs/33` §2 B1a）：落成 [`CONCEPT_UNNAMED_LINK`]
/// 占位，用户事后补；撤线只留给显式 `Esc`。
pub(crate) fn phrase_commit(
    concept_map: bool,
    text: &str,
    opening: &str,
    blank_new: bool,
) -> PhraseCommit {
    let t = text.trim();
    if !blank_new && t == opening.trim() {
        return PhraseCommit::Unchanged;
    }
    if concept_map && t.is_empty() {
        return if blank_new {
            PhraseCommit::Set(Some(CONCEPT_UNNAMED_LINK.to_string()))
        } else {
            PhraseCommit::RejectEmpty
        };
    }
    PhraseCommit::Set((!t.is_empty()).then(|| t.to_string()))
}

impl Canvas {
    /// 这份文档是不是概念图。概念图的拖拽、菜单、整理布局全按它分流。
    pub(crate) fn concept_map(&self) -> bool {
        self.session.doc.meta.layout.kind == "conceptMap"
    }

    /// 面板（右键菜单/检查器）用的公开判定：与 [`Canvas::concept_map`] 同源。
    pub fn is_concept_map(&self) -> bool {
        self.concept_map()
    }

    /// 给用户一句话提示（`docs/33` §5）。
    ///
    /// 工具条的禁用态**灰显但可点**——点了没反应等于按钮坏了，所以要说话：
    /// 「先选中一个概念」比一个不动声色的灰按钮有用得多。
    pub fn hint(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), false));
    }

    /// 概念图方向键微调：把选中的概念按世界像素挪一步（`docs/30` §5.7）。
    ///
    /// 为什么方向键在概念图里不是「换选下一个」：命题网没有前驱后继可言，
    /// 用户在概念图里按方向键，想的是「这个盒子往右一点」。步长由调用方按
    /// `Shift` 决定（1px / 8px），一次按键一条命令、一条撤销。
    pub fn nudge_concepts(&mut self, dx: f32, dy: f32) -> bool {
        if !self.concept_map() || (dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON) {
            return false;
        }
        let mut commands: Vec<Command> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        for &i in self.sel.ids() {
            let Some(id) = self.scene.ids.get(i as usize).cloned() else {
                continue;
            };
            let Some(node) = self.session.doc.node(&id) else {
                continue;
            };
            // 只有浮动概念有位置可言；万一文档里混进树节点，跳过它而不是整次失败。
            if node.parent_id.is_some() {
                continue;
            }
            let r = self.scene.rect(i);
            commands.push(Command::MoveConcept {
                id: id.clone(),
                x: r.x + dx,
                y: r.y + dy,
            });
            ids.push(id);
        }
        if commands.is_empty() {
            return false;
        }
        let cmd = if commands.len() == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        crate::trace::event(&format!(
            "concept-nudge n={} dx={dx:.1} dy={dy:.1}",
            ids.len()
        ));
        // 微调是刻意的动作，不闪「重叠」警示——那是给拖拽失手的。
        self.sel.replace(restore_selection(
            &ids,
            &self.scene,
            &self.session.doc.root_id,
        ));
        self.refocus_canvas = true;
        true
    }

    /// 概念图全选（`Ctrl+A`）：命题网里「全选」= 选中所有概念，本来就是常见的对齐/整理前置动作。
    /// 思维导图不接这个键——那里的选择语义受折叠与聚焦影响，不能一把抓。
    pub fn select_all_concepts(&mut self) -> bool {
        if !self.concept_map() {
            return false;
        }
        let mut idxs: Vec<u32> = Vec::new();
        for (i, id) in self.scene.ids.iter().enumerate() {
            if self
                .session
                .doc
                .node(id)
                .is_some_and(|n| n.parent_id.is_none())
            {
                idxs.push(i as u32);
            }
        }
        if idxs.is_empty() {
            return false;
        }
        let primary = self.sel.primary();
        if let Some(p) = primary.filter(|p| idxs.contains(p)) {
            idxs.retain(|&i| i != p);
            idxs.push(p);
        }
        if !self.sel.replace(idxs) {
            return false;
        }
        self.annotations.clear();
        self.relationship.clear();
        self.refocus_canvas = true;
        true
    }

    /// 视觉验收（`cm-node-edit` 场景）：按文字选中一个概念并进入就地改名。
    ///
    /// 走的是双击/F2 的同一个入口 [`Canvas::start_edit`]，只是省掉「先在屏幕上点中它」
    /// 这一步——脚本里的点击坐标会跟着相机 fit 漂（与 `MW_CONCEPT_SELECT` 同一个理由）。
    /// 「跳过手势、不跳过逻辑」：这段之后的一切（编辑器、草稿、提交、框宽重算）都是生产的。
    pub fn start_concept_edit(&mut self, text: &str) -> bool {
        let Some(id) = self.node_id_by_text(text) else {
            return false;
        };
        let Some(idx) = self.scene.find(&id) else {
            return false;
        };
        let viewport = self.last_viewport;
        self.start_edit(idx, viewport);
        self.editor.is_some()
    }

    /// 视觉验收（`cm-link-create` 场景）：按文字连一条线，**走 `commit_link`**。
    ///
    /// `commit_link` 就是鼠标在目标概念上松手那一步的函数——所以「新线落盘 → 立刻进
    /// 短语编辑态（空 → `＋ 链接词`）」这条 `docs/30` §5.9 的硬要求被这张图钉住。
    /// 指针几何（`side` / `at`）不参与：`commit_link` 只用 `from` 与 `to`。
    pub fn create_concept_link(&mut self, from_text: &str, to_text: &str) -> bool {
        if !self.concept_map() {
            return false;
        }
        let (Some(from_id), Some(to_id)) = (
            self.node_id_by_text(from_text),
            self.node_id_by_text(to_text),
        ) else {
            return false;
        };
        let (Some(from), Some(to)) = (self.scene.find(&from_id), self.scene.find(&to_id)) else {
            return false;
        };
        let l = Link {
            from,
            side: mw_ui::Side::Right,
            at: Point::new(0.0, 0.0),
            to: Some(to),
        };
        self.commit_link(&l)
    }

    /// 视觉验收（`cm-link-drag` 场景）：把拖拽**停在半途**，拍落空占位预览。
    ///
    /// 只立起 `self.link`，不建 `Drag`：`paint_link` 只看 `link` 有没有值，
    /// 所以这一帧就是「用户还捏着鼠标、指针在空白处」的真实画面。
    pub fn begin_concept_link_drag(&mut self, from_text: &str, dx: i32, dy: i32) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(from_id) = self.node_id_by_text(from_text) else {
            return false;
        };
        let Some(pos) = self
            .session
            .doc
            .node(&from_id)
            .and_then(|n| n.pos.as_ref())
            .map(|p| (p.x, p.y))
        else {
            return false;
        };
        let Some(idx) = self.scene.find(&from_id) else {
            return false;
        };
        self.link = Some(Link {
            from: idx,
            side: mw_ui::Side::Right,
            at: Point::new(pos.0 + dx as f32, pos.1 + dy as f32),
            to: None,
        });
        true
    }

    /// 视觉验收（`cm-link-blank` 场景，CM-10 ①）：从概念拖到**空白处**松手。
    ///
    /// 与 `create_concept_link` 只差一件事：`to` 是 `None`、`at` 是真落点。
    /// 所以这张图钉住的是「落空也要长出一个概念并连线」这条新手势，
    /// 而不是鼠标几何——落点由 `dx/dy` 相对源概念给出，跨机器可复现。
    pub fn create_concept_link_blank(&mut self, from_text: &str, dx: i32, dy: i32) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(from_id) = self.node_id_by_text(from_text) else {
            return false;
        };
        let Some(pos) = self
            .session
            .doc
            .node(&from_id)
            .and_then(|n| n.pos.as_ref())
            .map(|p| (p.x, p.y))
        else {
            return false;
        };
        let Some(from) = self.scene.find(&from_id) else {
            return false;
        };
        let l = Link {
            from,
            side: mw_ui::Side::Right,
            at: Point::new(pos.0 + dx as f32, pos.1 + dy as f32),
            to: None,
        };
        self.commit_link(&l)
    }

    /// 视觉验收（`cm-label-ime` 场景）：按链接短语打开已有连线的短语编辑框。
    ///
    /// 与双击连线的中点同一条 [`Canvas::start_rel_edit`]；`blank_new = false`，
    /// 所以这里 Esc 是还原、不是撤线——`cm-link-create` 那张才是「新线留空即撤」。
    pub fn edit_concept_phrase(&mut self, label: &str) -> bool {
        let Some(idx) = self
            .scene
            .relationships
            .iter()
            .position(|r| r.label.as_deref() == Some(label))
        else {
            return false;
        };
        let rel = &self.scene.relationships[idx];
        let sel = RelationshipSel {
            from: self.scene.ids[rel.from as usize].clone(),
            id: rel.id.clone(),
            to: self.scene.ids[rel.to as usize].clone(),
        };
        let viewport = self.last_viewport;
        self.start_rel_edit(sel, viewport, false)
    }

    /// 视觉验收（CM-14）：选中一颗概念并进入连线态（`MW_CONCEPT_LINK_MODE=<概念文字>`）。
    ///
    /// 走的是 `Ctrl+L` 的同一条 `begin_relationship_action()`，所以基线锁的是
    /// 「源概念强调环 + 提示条 + 下一次点击落地」这个真实状态，不是另画一份。
    pub fn begin_concept_link_mode(&mut self, text: &str) -> bool {
        if !self.select_concept_by_text(text) {
            return false;
        }
        self.begin_relationship_action()
    }

    /// 视觉验收（CM-14）：选中两颗概念后走「直连」（`MW_CONCEPT_LINK_TWO="A|B"`）。
    ///
    /// 结果应当是一条新命题 + **立刻立起的短语框**（空框「＋ 链接词」）。
    pub fn connect_concepts_by_text(&mut self, a: &str, b: &str) -> bool {
        let (Some(ia), Some(ib)) = (self.node_id_by_text(a), self.node_id_by_text(b)) else {
            return false;
        };
        self.select_many(&[ia.clone(), ib.clone()], &ia);
        self.refocus_canvas = true;
        self.begin_relationship_action()
    }

    /// 视觉验收（CM-17）：按短语选中**多条**命题（`MW_CONCEPT_MULTI_REL="支撑|带来"`）。
    ///
    /// 走的是 `toggle`，所以基线锁的就是「加选」这条真实路径，而不是另造一个选中集。
    pub fn select_relationships_by_labels(&mut self, labels: &[&str]) -> bool {
        self.relationship.clear();
        for label in labels {
            let Some(idx) = self
                .scene
                .relationships
                .iter()
                .position(|r| r.label.as_deref() == Some(*label))
            else {
                continue;
            };
            let r = &self.scene.relationships[idx];
            self.relationship.toggle(RelationshipSel {
                from: self.scene.ids[r.from as usize].clone(),
                id: r.id.clone(),
                to: self.scene.ids[r.to as usize].clone(),
            });
        }
        if self.relationship.is_empty() {
            return false;
        }
        self.sel = Selection::default();
        self.annotations.clear();
        self.refocus_canvas = true;
        true
    }

    /// 视觉验收（CM-16）：把某条关系设成「虚线 + 粗线 + 有注释」并选中它。
    ///
    /// 走三条真命令（不是直接改字段），所以基线同时锁住：数据落了盘、`Scene` 认了
    /// 这两个新字段、检查器能显示出来。`MW_CONCEPT_REL_STYLE=<短语>` 用它。
    pub fn style_relationship_by_label(&mut self, label: &str) -> bool {
        let Some(idx) = self
            .scene
            .relationships
            .iter()
            .position(|r| r.label.as_deref() == Some(label))
        else {
            return false;
        };
        let rel = &self.scene.relationships[idx];
        let from = self.scene.ids[rel.from as usize].clone();
        let id = rel.id.clone();
        let _ = self.session.doc.apply(&Command::SetRelationshipDashed {
            from: from.clone(),
            id: id.clone(),
            dashed: true,
        });
        let _ = self.session.doc.apply(&Command::SetRelationshipWidth {
            from: from.clone(),
            id: id.clone(),
            width: Some(2.5),
        });
        let _ = self.session.doc.apply(&Command::SetRelationshipNote {
            from: from.clone(),
            id: id.clone(),
            note: Some("2026Q3 的数据".into()),
        });
        self.rebuild();
        self.select_relationship(&from, &id);
        true
    }

    /// 视觉验收（CM-14）：选中某条关系并反向（`MW_CONCEPT_REVERSE=<短语>`）。
    pub fn reverse_relationship_by_label(&mut self, label: &str) -> bool {
        if !self.select_relationship_by_label(label) {
            return false;
        }
        self.reverse_selected_relationship()
    }

    /// 视觉验收（CM-14）：打开某条关系的短语框并预置一段输入（`MW_CONCEPT_SUGGEST="短语|输入"`）。
    ///
    /// 只改编辑态的草稿文本，**不落盘**：基线要锁的是「输入到一半时下拉里有什么」，
    /// 不是「短语被改成了什么」。
    pub fn edit_concept_phrase_typed(&mut self, label: &str, typed: &str) -> bool {
        if !self.edit_concept_phrase(label) {
            return false;
        }
        if let Some(ed) = self.rel_edit.as_mut() {
            ed.text = typed.to_string();
            ed.pick = 0;
        }
        true
    }

    /// 视觉验收（C15）：给示例概念图的一对概念补**第二条命题**。
    ///
    /// 「用户痛点 → 增长策略」的连线接近竖直：车道法向是水平的，只靠法向扇开时
    /// 两条 36 宽的短语会横向压在一起——正好用来拍「短语沿切线错开」这一版。
    /// 视觉验收（C21，打磨轮 63）：给「用户痛点 → 增长策略」这一对补 `extra` 条平行命题。
    ///
    /// `extra = 1` 就是 C15 的既有形态（连同模板自带的那条共 2 条，`cm-parallel-select-*` 锁着）；
    /// `extra = 2` 才凑得出 3 条车道——C21 的等距避让只在 n ≥ 3 时生效，没有 3 车道的图就没有像素证据。
    /// 短语取三种不同的短词，方便在基线里一眼数出车道顺序。
    pub fn add_concept_parallel_links(&mut self, extra: usize) -> bool {
        if !self.concept_map() {
            return false;
        }
        let (Some(from), Some(to)) = (
            self.node_id_by_text("用户痛点"),
            self.node_id_by_text("增长策略"),
        ) else {
            return false;
        };
        const LABELS: [&str; 3] = ["推动", "阻碍", "依赖"];
        let mut applied = false;
        for label in LABELS.iter().take(extra.max(1)) {
            let cmd = Command::AddRelationship {
                from: from.clone(),
                to: to.clone(),
                label: Some((*label).to_string()),
            };
            if self.session.doc.apply(&cmd).is_err() {
                break;
            }
            applied = true;
        }
        if !applied {
            return false;
        }
        self.rebuild();
        self.request_fit();
        true
    }

    /// 视觉验收（`cm-crosslink` 场景）：给示例概念图补两条**交叉**连接。
    ///
    /// 交叉是概念图的常态（`docs/30` §5.4 允许），此处的意义不是"美化"，
    /// 而是把「两条线可以穿过彼此且渲染稳定」这件事钉进基线——微弧仍挂账（CM-03 转出）。
    pub fn add_concept_cross_links(&mut self) -> bool {
        if !self.concept_map() {
            return false;
        }
        let pairs = [
            ("用户痛点", "竞品对比", "区别于"),
            ("增长策略", "技术方案", "倒逼"),
        ];
        let mut added = false;
        for (from_text, to_text, label) in pairs {
            let (Some(from), Some(to)) = (
                self.node_id_by_text(from_text),
                self.node_id_by_text(to_text),
            ) else {
                continue;
            };
            let cmd = Command::AddRelationship {
                from,
                to,
                label: Some(label.to_string()),
            };
            if self.session.doc.apply(&cmd).is_ok() {
                added = true;
            }
        }
        if added {
            self.rebuild();
            self.request_fit();
        }
        added
    }

    /// 视觉验收：按文字选中一个概念，**不动相机**。
    ///
    /// `MW_CONCEPT_SELECT` 用它；`select_node` 会把相机居中到节点，那样基线就
    /// 只能拍到那一个盒子——脚本里也就不用再算世界→屏幕坐标去点它。
    pub fn select_concept_by_text(&mut self, text: &str) -> bool {
        let Some(id) = self.node_id_by_text(text) else {
            return false;
        };
        let Some(idx) = self.scene.find(&id) else {
            return false;
        };
        self.select_node_only(idx, "concept-select");
        self.glow = None;
        self.refocus_canvas = true;
        true
    }

    /// 按文字找节点 id（概念图示例与演示入口共用）。
    fn node_id_by_text(&self, text: &str) -> Option<String> {
        self.session
            .doc
            .doc_order()
            .into_iter()
            .find(|id| self.session.doc.node(id).is_some_and(|n| n.text == text))
    }

    /// 概念图拖拽落点：**只写 `pos`**，一律不碰结构（`docs/30` §5.5）。
    ///
    /// 和树布局拖拽的三处差别：
    /// 1. 主概念也能拖（走 `MoveConcept`，`SetNodePos` 拒绝根）；
    /// 2. 落在别的概念上不改父级；
    /// 3. 落点与任何概念相交时，给这一批概念闪 0.25s 的 1px 告警边。
    pub(crate) fn commit_concept_move(&mut self, m: &Moving) -> bool {
        let zoom = self.camera.zoom.max(1e-4);
        let (dx, dy) = (m.offset.x / zoom, m.offset.y / zoom);
        if dx.abs() < 0.01 && dy.abs() < 0.01 {
            return false;
        }
        // Ctrl+拖拽：原件留在原地，副本落在拖拽位移处（与树内 `Ctrl+拖拽` 同一手感）。
        // 走概念专用命令，不把副本变成子节点。
        if m.copy {
            let ids: Vec<String> = m
                .roots
                .iter()
                .filter_map(|&i| self.scene.ids.get(i as usize).cloned())
                .collect();
            let nodes = concept_snapshot(&self.session.doc, &ids);
            let anchor = m
                .roots
                .first()
                .map(|&i| {
                    let r = self.scene.rect(i);
                    (r.x, r.y)
                })
                .unwrap_or((0.0, 0.0));
            let Some((cmd, tops)) = concept_clone_command(&nodes, (dx, dy), anchor) else {
                return false;
            };
            if self.session.doc.apply(&cmd).is_err() {
                return false;
            }
            self.rebuild();
            let sel: Vec<u32> = tops.iter().filter_map(|id| self.scene.find(id)).collect();
            self.sel.replace(sel);
            self.refocus_canvas = true;
            crate::trace::event(&format!(
                "concept-copy n={} dx={dx:.1} dy={dy:.1}",
                tops.len()
            ));
            return true;
        }
        let mut commands: Vec<Command> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        // 单颗概念的落点避让（`docs/33` §2 B3）：拖到别人头上就滑到最近的空位。
        // 多选整组一起动时**不**避让——组内本该保持相对位置，拆开就不是一次拖拽了。
        let solo = self.concept_map() && m.roots.len() == 1;
        for &i in &m.roots {
            let Some(id) = self.scene.ids.get(i as usize).cloned() else {
                continue;
            };
            let r = self.scene.rect(i);
            let (tx, ty) = if solo {
                self.free_concept_spot((r.x + dx, r.y + dy), (r.w, r.h), &m.roots)
            } else {
                (r.x + dx, r.y + dy)
            };
            commands.push(Command::MoveConcept {
                id: id.clone(),
                x: tx,
                y: ty,
            });
            ids.push(id);
        }
        if commands.is_empty() {
            return false;
        }
        let cmd = if commands.len() == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        crate::trace::event(&format!(
            "concept-move n={} dx={dx:.1} dy={dy:.1}",
            ids.len()
        ));
        let overlaps = self.overlapping_concepts(&ids);
        crate::trace::event(&format!("overlap_flash n={}", overlaps.len()));
        self.overlap_flash = (!overlaps.is_empty()).then_some((overlaps, self.now));
        self.sel.replace(restore_selection(
            &ids,
            &self.scene,
            &self.session.doc.root_id,
        ));
        self.refocus_canvas = true;
        true
    }

    /// 这一批概念里，与**别的**概念（含彼此）相交的那些 id。
    fn overlapping_concepts(&self, ids: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids {
            let Some(idx) = self.scene.find(id) else {
                continue;
            };
            let r = self.scene.rect(idx);
            for other in &self.scene.ids {
                if other == id {
                    continue;
                }
                let Some(oi) = self.scene.find(other) else {
                    continue;
                };
                if rects_overlap(r, self.scene.rect(oi)) {
                    out.push(id.clone());
                    break;
                }
            }
        }
        out
    }

    /// 把整理结果变成**一条**命令：位置没变的概念不出命令，多条合成 `Batch`（撤销栈里仍是一条）。
    ///
    /// 抽成纯函数是为了能直接测「一次撤销复原全部」，不必在单测里搭一整个 `Canvas`。
    /// 返回（命令，动过的颗数）——颗数给落地点弹提示用。
    pub(crate) fn arrange_batch(
        ids: &[String],
        current: &[(f32, f32)],
        placed: &[(f32, f32)],
    ) -> Option<(Command, usize)> {
        let mut commands: Vec<Command> = Vec::new();
        for (i, (x, y)) in placed.iter().enumerate() {
            let Some(id) = ids.get(i) else {
                continue;
            };
            let (ox, oy) = current.get(i).copied().unwrap_or((0.0, 0.0));
            if (x - ox).abs() < 0.01 && (y - oy).abs() < 0.01 {
                continue;
            }
            commands.push(Command::MoveConcept {
                id: id.clone(),
                x: *x,
                y: *y,
            });
        }
        let n = commands.len();
        if n == 0 {
            return None;
        }
        let cmd = if n == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        Some((cmd, n))
    }

    /// 「整理布局」（`docs/30` §5.5）：整图分层重排，**一条 Batch 一次撤销**。
    ///
    /// 概念集合取场景里所有浮动概念（`parent_id = None`，含主概念），
    /// 边取 `relationships`；算法本身在 `mw_core::concept_layout`，这里只负责
    /// 把几何喂进去、把结果变成命令。
    pub fn arrange_concepts(&mut self) -> Option<usize> {
        if !self.concept_map() {
            return None;
        }
        let doc_ids: Vec<String> = self
            .scene
            .ids
            .iter()
            .filter(|id| {
                self.session
                    .doc
                    .node(id)
                    .is_some_and(|n| n.parent_id.is_none())
            })
            .cloned()
            .collect();
        if doc_ids.len() < 2 {
            return None;
        }
        let mut slot: HashMap<&str, usize> = HashMap::new();
        for (i, id) in doc_ids.iter().enumerate() {
            slot.insert(id.as_str(), i);
        }
        let sizes: Vec<(f32, f32)> = doc_ids
            .iter()
            .map(|id| {
                self.scene
                    .find(id)
                    .map(|i| {
                        let r = self.scene.rect(i);
                        (r.w, r.h)
                    })
                    .unwrap_or((120.0, 36.0))
            })
            .collect();
        let current: Vec<(f32, f32)> = doc_ids
            .iter()
            .map(|id| {
                self.scene
                    .find(id)
                    .map(|i| {
                        let r = self.scene.rect(i);
                        (r.x, r.y)
                    })
                    .unwrap_or((0.0, 0.0))
            })
            .collect();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for (i, id) in doc_ids.iter().enumerate() {
            let Some(n) = self.session.doc.node(id) else {
                continue;
            };
            for rel in &n.relationships {
                if let Some(&j) = slot.get(rel.to.as_str()) {
                    edges.push((i, j));
                }
            }
        }
        let fixed: Vec<bool> = doc_ids
            .iter()
            .map(|id| self.is_concept_pinned(id))
            .collect();
        let placed = mw_core::concept_layout::arrange_scoped(
            mw_core::concept_layout::Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &fixed,
        );
        let (cmd, moved) = Self::arrange_batch(&doc_ids, &current, &placed)?;
        let pinned_count = fixed.iter().filter(|&&f| f).count();
        if pinned_count > 0 {
            crate::trace::event(&format!(
                "concept-arrange n={} moved={} pinned={}",
                doc_ids.len(),
                moved,
                pinned_count
            ));
        } else {
            crate::trace::event(&format!(
                "concept-arrange n={} moved={}",
                doc_ids.len(),
                moved
            ));
        }
        if self.session.doc.apply(&cmd).is_err() {
            return None;
        }
        self.rebuild();
        self.overlap_flash = None;
        // 记下「整理」落在栈里的位置：撤销踩到这一条时要把取景一起 `fit` 回去。
        self.arrange_undo_depth = Some(self.session.doc.undo_depth());
        self.arrange_redo_fit = false;
        // 整理是**整图**动作：结果必须落到视野里，否则用户点了「整理布局」只看到空白。
        self.request_fit();
        self.refocus_canvas = true;
        Some(moved)
    }

    /// 只整理当前选中的概念（未选中的概念与已钉住的概念均保持原位）。
    pub fn arrange_selected_concepts(&mut self) -> Option<usize> {
        if !self.concept_map() {
            return None;
        }
        let doc_ids: Vec<String> = self
            .scene
            .ids
            .iter()
            .filter(|id| {
                self.session
                    .doc
                    .node(id)
                    .is_some_and(|n| n.parent_id.is_none())
            })
            .cloned()
            .collect();
        if doc_ids.len() < 2 {
            return None;
        }
        let is_selected = |id: &str| -> bool {
            self.scene
                .find(id)
                .is_some_and(|idx| self.sel.contains(idx))
        };
        let selected_count = doc_ids.iter().filter(|id| is_selected(id)).count();
        if selected_count < 2 {
            return None;
        }
        let fixed: Vec<bool> = doc_ids
            .iter()
            .map(|id| !is_selected(id) || self.is_concept_pinned(id))
            .collect();
        if fixed.iter().all(|&f| f) {
            return None;
        }
        let mut slot: HashMap<&str, usize> = HashMap::new();
        for (i, id) in doc_ids.iter().enumerate() {
            slot.insert(id.as_str(), i);
        }
        let sizes: Vec<(f32, f32)> = doc_ids
            .iter()
            .map(|id| {
                self.scene
                    .find(id)
                    .map(|i| {
                        let r = self.scene.rect(i);
                        (r.w, r.h)
                    })
                    .unwrap_or((120.0, 36.0))
            })
            .collect();
        let current: Vec<(f32, f32)> = doc_ids
            .iter()
            .map(|id| {
                self.scene
                    .find(id)
                    .map(|i| {
                        let r = self.scene.rect(i);
                        (r.x, r.y)
                    })
                    .unwrap_or((0.0, 0.0))
            })
            .collect();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for (i, id) in doc_ids.iter().enumerate() {
            let Some(n) = self.session.doc.node(id) else {
                continue;
            };
            for rel in &n.relationships {
                if let Some(&j) = slot.get(rel.to.as_str()) {
                    edges.push((i, j));
                }
            }
        }
        let placed = mw_core::concept_layout::arrange_scoped(
            mw_core::concept_layout::Arrange {
                sizes: &sizes,
                edges: &edges,
                current: &current,
            },
            &fixed,
        );
        let (cmd, moved) = Self::arrange_batch(&doc_ids, &current, &placed)?;
        crate::trace::event(&format!(
            "concept-arrange-scoped n={} selected={} moved={}",
            doc_ids.len(),
            selected_count,
            moved
        ));
        if self.session.doc.apply(&cmd).is_err() {
            return None;
        }
        self.rebuild();
        self.overlap_flash = None;
        self.arrange_undo_depth = Some(self.session.doc.undo_depth());
        self.arrange_redo_fit = false;
        self.request_fit();
        self.refocus_canvas = true;
        Some(moved)
    }

    /// 概念是否处于钉住状态（手工位置受保护，整理布局时不移动）。
    pub fn is_concept_pinned(&self, id: &str) -> bool {
        self.session
            .doc
            .node(id)
            .is_some_and(|n| n.pinned == Some(true))
    }

    /// 切换单颗概念的钉住状态（单条命令、一次撤销）。
    pub fn toggle_concept_pinned(&mut self, id: &str) -> bool {
        if !self.concept_map() {
            return false;
        }
        let Some(n) = self.session.doc.node(id) else {
            return false;
        };
        let new_pinned = !n.pinned.unwrap_or(false);
        let cmd = Command::SetConceptPinned {
            id: id.to_string(),
            pinned: new_pinned,
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        crate::trace::event(&format!("concept-pin id={id} pinned={new_pinned}"));
        self.rebuild();
        true
    }

    /// 切换当前选中概念的钉住状态（全部已钉则全部取消，否则全部钉住；一次撤销）。
    pub fn toggle_concept_pinned_selected(&mut self) -> Option<bool> {
        if !self.concept_map() {
            return None;
        }
        let selected_ids = self.selected_concept_ids();
        if selected_ids.is_empty() {
            return None;
        }
        let all_pinned = selected_ids.iter().all(|id| self.is_concept_pinned(id));
        let new_pinned = !all_pinned;
        let cmds: Vec<Command> = selected_ids
            .into_iter()
            .map(|id| Command::SetConceptPinned {
                id,
                pinned: new_pinned,
            })
            .collect();
        let cmd = if cmds.len() == 1 {
            cmds.into_iter().next().unwrap()
        } else {
            Command::Batch { commands: cmds }
        };
        if self.session.doc.apply(&cmd).is_err() {
            return None;
        }
        crate::trace::event(&format!(
            "concept-pin-selected count={} pinned={}",
            self.selected_count(),
            new_pinned
        ));
        self.rebuild();
        Some(new_pinned)
    }

    /// 当前选中的概念节点 id（排除非概念、树节点）。
    pub fn selected_concept_ids(&self) -> Vec<String> {
        self.scene
            .ids
            .iter()
            .enumerate()
            .filter(|(idx, id)| {
                self.sel.contains(*idx as u32)
                    && self
                        .session
                        .doc
                        .node(id)
                        .is_some_and(|n| n.parent_id.is_none())
            })
            .map(|(_, id)| id.clone())
            .collect()
    }

    /// 对齐选中的自由主题（至少 2 条），一条 Batch、一次撤销。
    pub fn align_free_topics(&mut self, how: Align) -> bool {
        let topics = self.selected_free_topics();
        if topics.len() < 2 {
            return false;
        }
        let rects: Vec<Rect> = topics.iter().map(|t| t.1).collect();
        let targets = arrange::align(&rects, how);
        let mut moves = Vec::new();
        for ((id, rect, (px, py)), (nx, ny)) in topics.into_iter().zip(targets) {
            let (dx, dy) = (nx - rect.x, ny - rect.y);
            if dx.abs() > 0.01 || dy.abs() > 0.01 {
                moves.push((id, px + dx, py + dy));
            }
        }
        if moves.is_empty() {
            return false;
        }
        crate::trace::event(&format!("align {:?} {}", how, moves.len()));
        self.apply_free_moves(moves)
    }

    /// 等距分布选中的自由主题（至少 3 条）；首尾不动，只均分中间空隙。
    pub fn distribute_free_topics(&mut self, axis: Distribute) -> bool {
        let topics = self.selected_free_topics();
        if topics.len() < 3 {
            return false;
        }
        let rects: Vec<Rect> = topics.iter().map(|t| t.1).collect();
        let targets = arrange::distribute(&rects, axis);
        let mut moves = Vec::new();
        for ((id, rect, (px, py)), (nx, ny)) in topics.into_iter().zip(targets) {
            let (dx, dy) = (nx - rect.x, ny - rect.y);
            if dx.abs() > 0.01 || dy.abs() > 0.01 {
                moves.push((id, px + dx, py + dy));
            }
        }
        if moves.is_empty() {
            return false;
        }
        crate::trace::event(&format!("distribute {:?} {}", axis, moves.len()));
        self.apply_free_moves(moves)
    }

    pub fn set_boundary_color(&mut self, color: Option<&str>) -> bool {
        let anns: Vec<AnnotationSel> = self
            .annotations
            .iter()
            .filter(|a| a.kind == AnnotationKind::Boundary)
            .cloned()
            .collect();
        if anns.is_empty() {
            return false;
        }
        let mut commands = Vec::with_capacity(anns.len());
        for ann in &anns {
            crate::trace::event(&format!(
                "boundary-color {} {}",
                ann.id,
                color.unwrap_or("theme")
            ));
            commands.push(Command::SetBoundaryColor {
                parent: ann.parent.clone(),
                id: ann.id.clone(),
                color: color.map(str::to_string),
            });
        }
        let cmd = if commands.len() == 1 {
            commands.pop().expect("至少一条")
        } else {
            Command::Batch { commands }
        };
        self.apply_command(cmd)
    }

    /// 右键菜单/双击入口：打开关系线标签的画布内联编辑。
    pub fn edit_relationship_label_action(&mut self) -> bool {
        let Some(rel) = self.relationship.primary().cloned() else {
            return false;
        };
        let viewport = self.last_viewport;
        self.start_rel_edit(rel, viewport, false)
    }

    pub(crate) fn start_rel_edit(
        &mut self,
        sel: RelationshipSel,
        viewport: [f32; 2],
        blank_new: bool,
    ) -> bool {
        if self.editor.is_some() || self.summary_edit.is_some() {
            return false;
        }
        let Some(sr) = self
            .scene
            .relationships
            .iter()
            .find(|r| r.id == sel.id && self.scene.ids[r.from as usize] == sel.from)
        else {
            return false;
        };
        let text = sr.label.clone().unwrap_or_default();
        let s = self.camera.world_to_screen(sr.label_at, viewport);
        let origin = self.last_rect.min;
        let center = Pos2::new(origin.x + s.x, origin.y + s.y);
        let rect = egui::Rect::from_center_size(center, Vec2::new(180.0, 30.0));
        crate::trace::event(&format!(
            "rel-edit-open origin=({:.1},{:.1}) world=({:.1},{:.1}) screen=({:.1},{:.1}) zoom={:.3}",
            origin.x, origin.y, sr.label_at.x, sr.label_at.y, center.x, center.y, self.camera.zoom
        ));
        self.relationship.select_only(sel.clone());
        self.annotations.clear();
        self.sel = Selection::default();
        self.rel_edit = Some(RelEdit {
            sel,
            text: text.clone(),
            opening: text,
            focus: true,
            rect,
            ime: false,
            blank_new,
            pick: 0,
        });
        true
    }

    pub(crate) fn rel_editor_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(mut ed) = self.rel_edit.take() else {
            return false;
        };
        // R2（`docs/33` §2 B2）：编辑框**每帧**跟着短语走。
        // 改前 rect 在打开那一刻算死（`start_rel_edit` 里 `world_to_screen` 一次），
        // 编辑中一平移一缩放，框和文字就各走各的——用户看到的就是「文字在漂」。
        if let Some(sr) = self
            .scene
            .relationships
            .iter()
            .find(|r| r.id == ed.sel.id && self.scene.ids[r.from as usize] == ed.sel.from)
        {
            let s = self.camera.world_to_screen(sr.label_at, self.last_viewport);
            let center = Pos2::new(self.last_rect.min.x + s.x, self.last_rect.min.y + s.y);
            ed.rect = egui::Rect::from_center_size(center, ed.rect.size());
        }
        // IME 组合期间 Enter 是「选字」，不是「提交」——中文输入法里这是同一次按键。
        if let Some(v) = ui.input(|i| crate::canvas::ime_transition(&i.events)) {
            ed.ime = v;
        }
        let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let enter = !ed.ime && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let ink = color32(self.session.theme.ui.text);

        // ── 短语建议（CmapTools 的 linking-phrase 下拉，`docs/30` §5.4.2）──
        //
        // 必须在 `TextEdit` **之前**算：候选按「这一帧进来时」的文本过滤，↑↓/Tab 也要在
        // `TextEdit` 之前吃掉——否则方向键先被光标移动吃掉、Tab 先被焦点导航吃掉。
        let suggestions = if self.concept_map() && !ed.ime {
            mw_core::linking_phrases(&self.session.doc, &ed.text, Some(ed.opening.as_str()))
        } else {
            Vec::new()
        };
        let popup = (!suggestions.is_empty()).then(|| {
            crate::canvas::phrase_suggest_rect(ed.rect, suggestions.len(), self.last_rect)
        });
        let mut accept: Option<String> = None;
        if popup.is_some() {
            if ed.pick >= suggestions.len() {
                ed.pick = 0;
            }
            let up = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
            let down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
            if down {
                ed.pick = (ed.pick + 1) % suggestions.len();
            }
            if up {
                ed.pick = (ed.pick + suggestions.len() - 1) % suggestions.len();
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
                accept = suggestions.get(ed.pick).cloned();
            }
        }

        // 短语是「线在标签处断开」露出来的那一段字：只给画布底色，不要描边和内边距，
        // 有框就变成画布上飘着一个控件（`docs/30` §5.4）。
        let response = ui.put(
            ed.rect,
            egui::TextEdit::singleline(&mut ed.text)
                .id(egui::Id::unique(("mw-rel-edit", ed.sel.id.as_str())))
                // R3（`docs/33` §2 B2）：编辑框字体与渲染同源。
                // 改前用 `LIST_META`（12px），而短语胶囊按 `REL_LABEL_FONT`（13px）
                // 排版——提交后宽度重测，短语就跳一下。
                .font(FontId::proportional(mw_core::scene::REL_LABEL_FONT))
                .text_color(ink)
                .hint_text("＋ 链接词")
                .frame(
                    egui::Frame::NONE
                        .fill(color32(self.session.theme.ui.canvas_bg))
                        .inner_margin(egui::Margin::symmetric(4, 2)),
                )
                .desired_width(168.0),
        );
        if ed.focus {
            response.request_focus();
            ed.focus = false;
        }

        // 点建议行 = 采用。必须**在** `lost_focus` 之前判：这一击会让 `TextEdit` 失焦，
        // 若让失焦先提交，提交的就是「打到一半的前缀」，点中的那条反而被丢掉。
        let clicked = popup.and_then(|r| {
            let (pressed, pos) = ui.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
            let p = pos?;
            if !pressed || !r.contains(p) {
                return None;
            }
            const ROW_H: f32 = 24.0;
            const PAD: f32 = 4.0;
            let row = (((p.y - r.top() - PAD) / ROW_H).floor().max(0.0)) as usize;
            suggestions.get(row.min(suggestions.len() - 1)).cloned()
        });
        if let Some(rect) = popup {
            self.paint_phrase_suggest(ui, rect, &suggestions, ed.pick);
        }

        let cancel = esc;
        if cancel {
            // 显式 `Esc` = 放弃这条命题：把线一起撤掉。这是**唯一**的撤线路径——
            // `Enter`/失焦留空现在落「未命名」占位（`docs/33` §2 B1a）。
            if ed.blank_new {
                let _ = self.apply_command(Command::DeleteRelationship {
                    from: ed.sel.from.clone(),
                    id: ed.sel.id.clone(),
                });
            }
            return true;
        }
        let picked = clicked.or(accept);
        if picked.is_some() || enter || response.lost_focus() {
            let commit_text = picked.clone().unwrap_or_else(|| ed.text.clone());
            if let Some(text) = picked {
                crate::trace::event(&format!(
                    "rel-suggest-pick from={} id={} text={text}",
                    ed.sel.from, ed.sel.id
                ));
            }
            match phrase_commit(self.concept_map(), &commit_text, &ed.opening, ed.blank_new) {
                PhraseCommit::Unchanged => true,
                PhraseCommit::RejectEmpty => {
                    crate::trace::event(&format!(
                        "rel-label-reject reason=empty from={} id={}",
                        ed.sel.from, ed.sel.id
                    ));
                    self.notice = Some(("概念图的连接短语不能为空".into(), false));
                    true
                }
                PhraseCommit::Set(label) => {
                    crate::trace::event(&format!("rel-label {} {}", ed.sel.from, ed.sel.id));
                    self.apply_command(Command::SetRelationshipLabel {
                        from: ed.sel.from,
                        id: ed.sel.id,
                        label,
                    })
                }
            }
        } else {
            self.rel_edit = Some(ed);
            false
        }
    }

    /// 画短语建议下拉（`docs/30` §5.4.2）：面板底 + 1px 描边，当前行铺一层强调色。
    ///
    /// 纯绘制、不注册交互——点击由 `rel_editor_ui` 按几何判行，避免和 `TextEdit` 抢焦点。
    fn paint_phrase_suggest(&self, ui: &egui::Ui, rect: egui::Rect, rows: &[String], pick: usize) {
        const ROW_H: f32 = 24.0;
        const PAD: f32 = 4.0;
        let t = &self.session.theme;
        let p = ui.painter();
        p.rect_filled(rect, 8.0, color32(t.ui.panel2));
        p.rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(1.0, color32(t.ui.border)),
            egui::StrokeKind::Inside,
        );
        let a = t.ui.accent;
        for (i, text) in rows.iter().enumerate() {
            let row = egui::Rect::from_min_size(
                egui::Pos2::new(rect.left() + PAD, rect.top() + PAD + i as f32 * ROW_H),
                egui::vec2(rect.width() - PAD * 2.0, ROW_H),
            );
            if i == pick {
                p.rect_filled(
                    row,
                    4.0,
                    Color32::from_rgba_unmultiplied(a.0[0], a.0[1], a.0[2], 26),
                );
            }
            p.text(
                egui::pos2(row.left() + 8.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                text,
                FontId::proportional(design::content::LIST_EXCERPT.size),
                color32(t.ui.text),
            );
        }
    }

    /// 视觉验收夹具：给当前概念图写上聚焦问题（只给 golden 钩子用）。
    ///
    /// 直接写 `meta` 而不是走命令：走命令会把文档标脏，标题栏多出「未保存」标记、
    /// 状态栏也跟着变——`cm-focus-question` 就会同时锁住那两处像素，基线不再只讲问题带。
    /// 命令路径（归一化 / 撤销 / 序列化）由 `focus_question_is_normalised_and_undoable`
    /// 与 `mw-format` 的两条兼容测试覆盖，不靠截图。
    pub fn set_focus_question_for_demo(&mut self, text: &str) {
        let trimmed = text.trim();
        self.session.doc.meta.layout.focus_question =
            (!trimmed.is_empty()).then(|| trimmed.to_string());
        self.rebuild();
    }
    /// 当前图里写着的聚焦问题（大纲页的就地编辑钩子要用）。
    pub(crate) fn focus_question(&self) -> Option<String> {
        self.session.doc.meta.layout.focus_question.clone()
    }
    /// 视觉验收夹具：把当前图里所有关系线的箭头改成 `head`（⑦ 的截图用）。
    ///
    /// 同上：直接改文档，不走命令——夹具在标题栏留下「未保存」就会让基线多锁两处像素。
    pub fn set_arrow_head_for_demo(&mut self, head: mw_core::model::ArrowHead) {
        for node in self.session.doc.nodes.values_mut() {
            for r in node.relationships.iter_mut() {
                r.head = head;
            }
        }
        self.rebuild();
    }
    /// 打开「聚焦问题」的就地编辑（双击画布顶部那行字）。
    pub(crate) fn start_focus_edit(&mut self, rect: egui::Rect) -> bool {
        if self.focus_edit.is_some() || self.editor.is_some() {
            return false;
        }
        let Some(text) = self.session.doc.meta.layout.focus_question.clone() else {
            return false;
        };
        self.focus_edit = Some(FocusEdit {
            text: text.clone(),
            opening: text,
            focus: true,
            rect,
            ime: false,
        });
        true
    }

    /// 画布顶部的「聚焦问题」一行字（CM-10 ⑥，`docs/32` §2）。
    ///
    /// 只在真的写了问题时才画：没写的概念图**一个像素都不多**，现有 `cm-*` 基线
    /// 因此逐张不变。带子画在壳层、不进 `Scene`，几何/布局/导出都不受影响。
    pub(crate) fn paint_focus_question(
        &mut self,
        ui: &egui::Ui,
        origin: egui::Rect,
        _viewport: [f32; 2],
    ) {
        self.focus_band = None;
        if !self.scene.kind.is_concept_map()
            || self.reading
            || self.focus_edit.is_some()
            || self.editor.is_some()
        {
            return;
        }
        let Some(text) = self.session.doc.meta.layout.focus_question.clone() else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        let font = FontId::proportional(design::content::READING_BODY.size);
        // 按画布宽换行，不排成一行：一行字会被窗口左右硬切，而问题可能问得很长。
        let wrap = focus_wrap_width(origin);
        let galley = ui
            .painter()
            .layout(text, font, egui::Color32::PLACEHOLDER, wrap);
        let band = focus_band_rect(origin, galley.size());
        self.focus_band = Some(band);
        let response = ui.interact(
            band,
            ui.scope_id().with("mw-focus-question"),
            egui::Sense::click(),
        );
        let hovered = response.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        let painter = ui.painter_at(origin);
        if hovered {
            painter.rect_filled(
                band,
                design::radius::MD,
                color32(self.session.theme.ui.hover),
            );
        }
        // 文字居中：聚焦问题是「这张图在回答什么」的一句话，居中最像话，不像标签。
        let pos = band.center() - galley.size() * 0.5;
        let ink = if hovered {
            self.session.theme.ui.text
        } else {
            self.session.theme.ui.dim
        };
        painter.galley(pos, galley, color32(ink));
        // 悬停才补底线：不悬停就只是一行字，不画框——问题带不该抢主体的视线。
        if hovered {
            let y = band.bottom() - 4.0;
            painter.line_segment(
                [
                    Pos2::new(band.left() + 12.0, y),
                    Pos2::new(band.right() - 12.0, y),
                ],
                egui::Stroke::new(1.0, color32(self.session.theme.ui.accent)),
            );
        }
        if response.double_clicked() {
            self.start_focus_edit(band);
        }
    }

    /// 「聚焦问题」的行内编辑框：Enter 提交、Esc 取消、点走也提交。
    ///
    /// IME 组合期间 Enter 是选字（与关系线标签同一条规则），不能当提交。
    pub(crate) fn focus_editor_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(mut ed) = self.focus_edit.take() else {
            return false;
        };
        if let Some(v) = ui.input(|i| crate::canvas::ime_transition(&i.events)) {
            ed.ime = v;
        }
        let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let enter = !ed.ime && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        // 控件本身住在 `content_kit`：大纲页那条带子用的是同一个（docs/32 §8）。
        let response = crate::content_kit::question_field(
            ui,
            ed.rect,
            &mut ed.text,
            color32(self.session.theme.ui.text),
            color32(self.session.theme.ui.canvas_bg),
            egui::Id::unique("mw-focus-edit"),
        );
        if ed.focus {
            response.request_focus();
            ed.focus = false;
        }
        let done = enter || response.lost_focus();
        match crate::content_kit::question_edit_outcome(&ed.text, &ed.opening, esc, done) {
            crate::content_kit::QuestionEditOutcome::Continue => {
                self.focus_edit = Some(ed);
                false
            }
            crate::content_kit::QuestionEditOutcome::Cancel => true,
            crate::content_kit::QuestionEditOutcome::Commit(text) => {
                crate::trace::event(&format!("focus-question text={:?}", text));
                self.apply_command(Command::SetFocusQuestion {
                    text: (!text.is_empty()).then_some(text),
                })
            }
        }
    }

    pub fn edit_summary_action(&mut self) -> bool {
        let Some(ann) = self.annotations.last().cloned() else {
            return false;
        };
        let viewport = self.last_viewport;
        self.start_summary_edit(&ann, viewport)
    }

    pub(crate) fn start_summary_edit(&mut self, ann: &AnnotationSel, viewport: [f32; 2]) -> bool {
        if ann.kind != AnnotationKind::Summary || self.editor.is_some() || self.rel_edit.is_some() {
            return false;
        }
        let Some(s) = self.scene.summaries.iter().find(|s| s.id == ann.id) else {
            return false;
        };
        let Some(text) = self
            .session
            .doc
            .node(&ann.parent)
            .and_then(|n| n.summaries.iter().find(|x| x.id == ann.id))
            .map(|x| x.text.clone())
        else {
            return false;
        };
        let a = self
            .camera
            .world_to_screen(Point::new(s.topic.x, s.topic.y), viewport);
        let b = self
            .camera
            .world_to_screen(Point::new(s.topic.right(), s.topic.bottom()), viewport);
        let origin = self.last_rect.min;
        let rect = egui::Rect::from_min_max(
            Pos2::new(origin.x + a.x, origin.y + a.y),
            Pos2::new(origin.x + b.x, origin.y + b.y),
        )
        .expand(6.0);
        let rect = rect.union(egui::Rect::from_min_size(rect.min, Vec2::new(160.0, 34.0)));
        self.annotations = vec![ann.clone()];
        self.relationship.clear();
        self.sel = Selection::default();
        self.summary_edit = Some(SummaryEdit {
            parent: ann.parent.clone(),
            id: ann.id.clone(),
            text: text.clone(),
            opening: text,
            focus: true,
            rect,
        });
        true
    }

    /// 每帧画概要编辑器；返回是否改动了文档（需要重排/重绘）。
    ///
    /// 提交规则：`Ctrl+Enter` 或失焦提交，`Esc` 取消；文字没变就不发命令。
    /// 每帧画概要编辑器；返回是否改动了文档（需要重排/重绘）。
    ///
    /// 提交规则：`Ctrl+Enter` 或失焦提交，`Esc` 取消；文字没变就不发命令。
    pub(crate) fn summary_editor_ui(&mut self, ui: &mut egui::Ui, viewport: [f32; 2]) -> bool {
        let Some(mut ed) = self.summary_edit.take() else {
            return false;
        };
        let Some(s) = self.scene.summaries.iter().find(|s| s.id == ed.id) else {
            return true;
        };
        let a = self
            .camera
            .world_to_screen(Point::new(s.topic.x, s.topic.y), viewport);
        let b = self
            .camera
            .world_to_screen(Point::new(s.topic.right(), s.topic.bottom()), viewport);
        let origin = self.last_rect.min;
        let rect = egui::Rect::from_min_max(
            Pos2::new(origin.x + a.x, origin.y + a.y),
            Pos2::new(origin.x + b.x, origin.y + b.y),
        )
        .expand(6.0);
        ed.rect = rect.union(egui::Rect::from_min_size(rect.min, Vec2::new(160.0, 34.0)));

        let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        // Enter 提交、Shift+Enter 换行（多行概要用得上）；Ctrl+Enter 也提交，照顾习惯。
        let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let commit_key = enter || ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let font = FontId::proportional(
            (mw_core::scene::SUMMARY_FONT * self.camera.zoom).clamp(11.0, 28.0),
        );
        let ink = color32(self.session.theme.ui.text);
        let response = ui.put(
            ed.rect,
            egui::TextEdit::multiline(&mut ed.text)
                .id(egui::Id::unique(("mw-summary-edit", ed.id.as_str())))
                .font(font)
                .text_color(ink)
                .frame(
                    egui::Frame::NONE
                        .fill(with_alpha(self.session.theme.ui.panel, 252))
                        .stroke(egui::Stroke::new(
                            design::stroke::HAIRLINE,
                            with_alpha(self.session.theme.ui.accent, 180),
                        ))
                        .inner_margin(egui::Margin::same(6)),
                )
                .desired_width((ed.rect.width() - 12.0).max(80.0))
                .char_limit(MAX_TEXT_LEN),
        );
        if ed.focus {
            response.request_focus();
            ed.focus = false;
        }
        if esc {
            crate::trace::event(&format!("summary-edit cancel {}", ed.id));
            return true;
        }
        if commit_key || response.lost_focus() {
            let changed = ed.text != ed.opening;
            if changed {
                crate::trace::event(&format!("summary-edit commit {}", ed.id));
                return self.apply_command(Command::SetSummaryText {
                    parent: ed.parent,
                    id: ed.id,
                    text: ed.text,
                });
            }
            return true;
        }
        self.summary_edit = Some(ed);
        false
    }

    pub(crate) fn hit_at(&self, local: Option<Point>, viewport: [f32; 2]) -> Option<Target> {
        local.and_then(|p| {
            let world = self.camera.screen_to_world(p, viewport);
            self.hit.hit(self.camera, world)
        })
    }

    /// 单击折叠圆点：折叠 ↔ 展开整棵。返回「这一击是否真的改了文档」。
    ///
    /// 返回值只给 M32 的双击识别用：单击没改状态（本来就是这个折叠态）时，
    /// 双击不能去撤销栈里撤别人的命令。
    pub(crate) fn toggle(&mut self, node: u32) -> bool {
        let Some(id) = self.scene.ids.get(node as usize).cloned() else {
            return false;
        };
        let collapsed = self.scene.node(node).collapsed;
        // 折叠圆点属于「对这个节点动手」，不保留多选组，也不许关系线/注解还选着。
        self.select_node_only(node, "fold-dot");
        let Ok(cs) = self.session.doc.apply(&Command::SetCollapsed {
            id: id.clone(),
            collapsed: !collapsed,
        }) else {
            return false;
        };
        let pushed = !cs.is_empty();
        self.rebuild();
        // 打磨轮 8：圆点 vs 盒子边界的命中带只有几个像素，脚本点没点中很难从截图看出来。
        if crate::trace::is_enabled() {
            crate::trace::event(&format!(
                "toggle idx={node} collapsed={} kids={}",
                !collapsed,
                self.session
                    .doc
                    .node(&id)
                    .map(|n| n.children.len())
                    .unwrap_or(0)
            ));
        }
        pushed
    }

    /// 双击折叠圆点「展开到下一级」（M32 / `docs/02` §3）。
    ///
    /// 第一击已经按单击落地（可能折了、可能展了）。双击到达时先把它**撤掉**——
    /// 回到第一击之前的状态——再用 `mw_core::drill` 的下钻规划下发一条命令。
    /// 这样双击前后只多一条撤销记录，而不是「单击一次 + 下钻一次」两条。
    ///
    /// `first` 是 `mw_ui::drill_click` 记住的第一击现场；`first.undo_depth` 是
    /// 第一击落地后的栈深，用来当安全阀：中间若落了别的命令，就不当双击。
    pub(crate) fn toggle_drill(&mut self, node: u32, first: &ToggleClick) -> bool {
        let Some(id) = self.scene.ids.get(node as usize).cloned() else {
            return false;
        };
        if first.pushed {
            if self.session.doc.undo_depth() != first.undo_depth {
                return false;
            }
            if !self.undo() {
                return false;
            }
        }
        let Some(cmd) = mw_core::drill::drill_toggle_command(&self.session.doc, &id) else {
            if first.pushed {
                // 双击本来就不改变状态（如子级全展开）：把第一击撤干净，
                // 也别让 `Ctrl+Y` 把被取消的那一击重做回来。
                self.session.doc.clear_redo();
                crate::trace::event(&format!("toggle_drill id={id} depth=0 open=0"));
            }
            return first.pushed;
        };
        // 轨迹里的数：动了几个直接子级、自己有没有被展开。
        let (depth, open) = match &cmd {
            Command::SetCollapsed {
                id: target,
                collapsed,
            } => {
                let open = target == &id && !*collapsed;
                (usize::from(!open), open)
            }
            Command::Batch { commands } => {
                let mut depth = 0;
                let mut open = false;
                for c in commands {
                    if let Command::SetCollapsed {
                        id: target,
                        collapsed,
                    } = c
                    {
                        if target == &id {
                            open = !*collapsed;
                        } else {
                            depth += 1;
                        }
                    }
                }
                (depth, open)
            }
            _ => (0, false),
        };
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        crate::trace::event(&format!(
            "toggle_drill id={id} depth={depth} open={}",
            u8::from(open)
        ));
        true
    }

    /// 编辑中的文本变化后，按新文本重排场景（节点盒实时增长）。
    pub(crate) fn sync_editing_layout(&mut self) {
        let Some(ed) = self.editor.as_ref() else {
            return;
        };
        if ed.laid_out == ed.text {
            return;
        }
        self.rebuild();
        let laid_out = self.editor.as_ref().map(|e| e.text.clone());
        if let (Some(ed), Some(text)) = (self.editor.as_mut(), laid_out) {
            ed.laid_out = text;
        }
        self.hovered = None;
        self.hover_detail = None;
        // 打字也会改变节点盒尺寸：长标题向屏幕外长时同样要把编辑器拉回来。
        // `reveal_node` 只在盒越界时做最小平移，正常输入时画面是静止的。
        if let Some(idx) = self.editor.as_ref().and_then(|e| self.scene.find(&e.id)) {
            self.reveal_node(idx);
        }
    }

    /// 当前布局根：聚焦时是被聚焦的子树根，否则是文档主根。
    pub(crate) fn layout_root(&self) -> String {
        self.focus
            .last()
            .cloned()
            .unwrap_or_else(|| self.session.doc.root_id.clone())
    }

    pub(crate) fn rebuild(&mut self) {
        let started = std::time::Instant::now();
        // 动画中途又改结构：把此刻屏幕上的那一帧固化成新起点，避免位置回跳。
        // 行内编辑期间不启动新过渡（编辑器要贴着节点走），已在飞的动画照常收尾。
        let from = if self.motion_reduced() || self.editor.is_some() {
            None
        } else if motion_pin().is_some() && self.motion.is_some() {
            // 定格（`MW_MOTION_T`）下不重取起点：`Motion::scene` 已经按钉死的 t 插过值，
            // 再把它当新起点插一次就成了 t²（钉 0.5 拍出来是 0.75），定格就不"半"了。
            // 只有在设了这个变量时才走这条路，正常运行一行都不多判。
            None
        } else {
            Some(match self.motion.as_ref() {
                Some(m) if m.active() => m.scene(&self.scene),
                _ => self.scene.clone(),
            })
        };
        // 布局与主题以 `doc.meta` 为准：`SetLayout`/`SetTheme`/`SetThemeAccent` 只改 meta，
        // 撤销/重做也会把旧 meta 还回来。会话里的两份镜像必须在这里跟住，
        // 否则「换主题」要等下次打开文件才生效。强调色覆盖同样住在 meta 里，
        // 所以这里必须走 `theme_for` 而不是 `get_theme(theme)`。
        self.session.settings = self.session.doc.meta.layout.clone();
        self.session.theme = theme_for(&self.session.doc.meta);
        // 聚焦根可能被删除/撤销掉了：逐级退回上一级，而不是让场景渲染一棵不存在的树。
        while self
            .focus
            .last()
            .is_some_and(|id| self.session.doc.node(id).is_none())
        {
            self.focus.pop();
        }
        let layout_root = self.layout_root();
        // 场景索引是布局 DFS 前序，重排后同一个 id 的索引会变——所以搬运的是 **id**，
        // 不是索引。搬不过去的（被删/被折叠藏起来的）直接丢掉，别让选择悬空。
        let old: Vec<String> = self
            .sel
            .ids()
            .iter()
            .filter_map(|&i| self.scene.ids.get(i as usize).cloned())
            .collect();
        let editing = self.editor.as_ref().map(|e| (e.id.clone(), e.text.clone()));
        let scene = mw_core::build_scene_cached(
            &self.session.doc,
            &layout_root,
            &self.session.settings,
            &self.session.theme,
            &*self.measure,
            editing
                .as_ref()
                .map(|(id, text)| (id.as_str(), text.as_str())),
            &self.scene_cache,
        );
        self.hit = HitIndex::new(&scene);
        self.scene = scene;
        let restored = restore_selection(&old, &self.scene, &layout_root);
        self.sel.replace(restored);
        // 关系线选中态也要跟着重建后的场景走：反向连接、撤销/重做都会让一条边换主人
        // （`from` 变了），旧的 `(from, id)` 在新场景里查不到。留着它，检查器会指着
        // 一条不存在的线，`Delete` 还会拿错 `from` 去删——直接清掉。
        // 多选时逐条清：一条边消失不该把别的选中一起带走。
        let scene = &self.scene;
        self.relationship.retain_alive(|rel| {
            scene
                .relationships
                .iter()
                .any(|r| r.id == rel.id && scene.ids.get(r.from as usize) == Some(&rel.from))
        });
        self.hovered = None;
        self.hover_detail = None;
        if self.motion_reduced() {
            self.motion = None;
        } else if let Some(from) = from {
            if let Some(m) = Motion::new(from, &self.scene) {
                self.motion = Some(m);
            }
        }
        // 重建轨迹（打磨轮 6）：一次重建到底花多久、一次操作触发几次。
        // 平移/缩放只动相机，不该出现在这条线里——「平移不重排」就靠它证。
        if crate::trace::is_enabled() {
            crate::trace::event(&format!(
                "scene_build nodes={} edges={} ms={:.2}",
                self.scene.nodes.len(),
                self.scene.edges.len(),
                started.elapsed().as_secs_f64() * 1000.0
            ));
        }
    }

    pub(crate) fn start_edit(&mut self, idx: u32, viewport: [f32; 2]) {
        let Some(id) = self.scene.ids.get(idx as usize).cloned() else {
            return;
        };
        let Some(node) = self.scene.nodes.get(idx as usize) else {
            return;
        };
        // 屏幕字号太小先把画布放大（围绕节点中心），否则编辑器与候选窗都不可用。
        let min_zoom = EDIT_MIN_PX / node.geom.font_size.max(1.0);
        if self.camera.zoom < min_zoom {
            let world_center = node.geom.rect.center();
            let before = self.camera.world_to_screen(world_center, viewport);
            self.camera.zoom = min_zoom.min(MAX_ZOOM);
            let after = self.camera.world_to_screen(world_center, viewport);
            self.camera.center.x += (after.x - before.x) / self.camera.zoom;
            self.camera.center.y += (after.y - before.y) / self.camera.zoom;
        }
        let text = self
            .session
            .doc
            .node(&id)
            .map(|n| n.text.clone())
            .unwrap_or_default();
        // 进编辑只处理这一个节点：组内成员只提为主选，不拆组。
        let _ = self.sel.click(idx, false);
        self.editor = Some(Editor {
            id,
            original: text.clone(),
            text: text.clone(),
            laid_out: text,
            focused: false,
            ime: false,
            fresh: false,
            fresh_depth: None,
        });
        // 新建/改名一进入编辑就把节点拉回视口：连着按 Tab 建子节点时，新节点
        // 会一路往右长出去；不跟住的话用户对着屏幕边建了四五层，却什么都看不见。
        self.reveal_node(idx);
    }

    pub(crate) fn commit_edit(&mut self) {
        let Some(ed) = self.editor.take() else {
            return;
        };
        if ed.text != ed.original {
            // 这枚节点若是这次编辑顺手建出来的（`Tab` / `Enter` / 悬停「＋」），
            // 名字要并进那条「新建节点」里去：否则按一次 `Ctrl+Z` 只清掉刚写的字，
            // 树上留一个空盒子——用户要撤的明明是「刚建的这个」（M8）。
            let fresh = ed.fresh;
            let id = ed.id.clone();
            let _ = self.session.doc.apply_coalesced(
                &Command::SetText {
                    id: ed.id,
                    text: ed.text,
                },
                |prev| fresh && prev.created(&id),
            );
        }
        self.rebuild();
        self.refocus_canvas = true;
    }

    pub(crate) fn cancel_edit(&mut self) {
        let Some(ed) = self.editor.take() else {
            return;
        };
        // 这次编辑顺手建出来的节点，取消时连它一起收回去。
        // 留着的话树上会多一个空白盒子——它选得中、但没有任何内容可读，
        // 而且用户刚刚明确表达了「不要这个」。写过字也一样收：那些字从没提交过，
        // `Esc` 的语义就是「还原」，而这里要还原的整个动作就是「建了这枚节点」。
        if ed.fresh {
            let parent = self
                .session
                .doc
                .node(&ed.id)
                .and_then(|n| n.parent_id.clone());
            let popped = cancel_fresh_node(&mut self.session.doc, &ed.id, ed.fresh_depth);
            crate::trace::event(&format!(
                "edit_cancel fresh id={} popped_undo={popped}",
                ed.id
            ));
            self.rebuild();
            // 选中退回到它的父节点：接着按 `Tab` 就能重来一次。
            let back = parent
                .as_deref()
                .and_then(|p| self.scene.find(p))
                .into_iter()
                .collect();
            self.sel.replace(back);
            self.refocus_canvas = true;
            return;
        }
        self.rebuild();
        self.refocus_canvas = true;
    }

    /// 非编辑态的键盘语义。只在画布持有焦点时生效，避免点了标题栏还在这里加节点。
    ///
    /// 浮层开着时键盘归浮层：这里的 Tab / Enter / Esc 是「加子节点 / 加同级节点 / 清空选择」，
    /// 而浮层的这三个键是「上下移动 / 确认 / 取消」。浮层是在画布**之后**画的，抢不过这里，
    /// 所以按不下去的是浮层——导出对话框的「回车确认」曾经就是这样失灵，反而把选择清空了。
    pub(crate) fn shortcuts(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        viewport: [f32; 2],
    ) -> bool {
        if !response.has_focus() || self.input_blocked {
            return false;
        }
        // 画布自己的确认框立着时，键盘归它：`Esc` 由 `confirm_delete_ui` 接（那一块在
        // 本函数之后跑）。这里若不收手，一次 `Esc` 会既取消弹窗又清掉选择。
        if self.confirm_delete.is_some() {
            return false;
        }
        let mut dirty = false;

        // 剪贴板和弦（Ctrl+C/X/V 与 Ctrl+Shift+C/V）不在这里：那段和撤销一样要求画布
        // 握着焦点，鼠标点一下工具条就把焦点交出去了。它归 `App::window_shortcuts` 管，
        // 见 `Canvas::apply_clipboard`。
        //
        // `consume_key` 的修饰键匹配会忽略多余的 Shift/Alt，所以**带 Shift 的组合必须先查**。
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let alt = Modifiers::ALT;
        if ui.input_mut(|i| i.consume_key(cmd_shift, Key::ArrowLeft)) {
            dirty |= self.promote_selected();
        } else if ui.input_mut(|i| i.consume_key(cmd_shift, Key::ArrowRight)) {
            dirty |= self.demote_selected();
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::D)) {
            dirty |= self.duplicate_selected();
        } else if ui.input_mut(|i| i.consume_key(alt, Key::ArrowUp)) {
            dirty |= self.move_selected(-1);
        } else if ui.input_mut(|i| i.consume_key(alt, Key::ArrowDown)) {
            dirty |= self.move_selected(1);
        } else if ui.input_mut(|i| {
            // Shift 按下时 winit 报的是 `?` 字符，egui 映射为 Questionmark。
            i.consume_key(cmd_shift, Key::Slash) || i.consume_key(cmd_shift, Key::Questionmark)
        }) {
            dirty |= self.set_all_collapsed(false);
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::Slash)) {
            dirty |= self.set_all_collapsed(true);
        } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Slash)) {
            dirty |= self.toggle_selected_collapsed();
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::Num0)) {
            // 复位与缩放条上的百分比是**同一条路**（X21）：以前这里直接写
            // `camera.zoom = 1.0`，绕过了 `zoom_reset_action` 的钳位与 `zoom_anchor` 轨迹。
            dirty |= self.zoom_reset_action();
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::Num1)) {
            self.fit_view();
            dirty = true;
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::Num2)) {
            dirty |= self.fit_selection();
        } else if ui.input_mut(|i| i.consume_key(alt, Key::Enter)) {
            // 必须在普通 Enter（加同级）之前吃掉：修饰键匹配同样忽略多余的 Alt。
            dirty |= self.focus_selected();
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::A)) {
            // 概念图：只选顶层概念（命题网没有层级，一把抓就是全部）。
            // 思维导图：选当前**可见**的全部节点——折叠起来的本来就不在 `Scene` 里，
            // 所以「不跨折叠」是天然成立的，不需要再查一遍树。
            dirty |= if self.concept_map() {
                self.select_all_concepts()
            } else {
                self.select_all_visible()
            };
        } else if ui.input_mut(|i| i.consume_key(cmd, Key::L)) {
            dirty |= self.begin_relationship_action();
        } else if self.concept_map() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2)) {
            // `docs/33` §5：F2 = 改文字。选中一条命题就改短语，选中概念就改概念文字
            // （右键菜单里那两条 F2 是同一个口径，这里只是把键真接上）。
            dirty |= if self.relationship.is_empty() {
                self.edit_primary_action()
            } else {
                self.edit_relationship_label_action()
            };
        } else if self.concept_map() && ui.input_mut(|i| i.consume_key(cmd, Key::P)) {
            // `docs/33` §5：Ctrl+P = 钉住位置。
            dirty |= self.toggle_concept_pinned_selected().is_some();
        } else if let Some(add) = take_annotation_shortcut(ui) {
            dirty |= self.add_annotation_from_keyboard(add);
        } else if let Some((dx, dy)) = take_arrow(ui) {
            if self.concept_map() {
                // 概念图：方向键是摆位，不是遍历（`docs/30` §5.7）。
                let step = nudge_step(ui.input(|i| i.modifiers.shift));
                dirty |= self.nudge_concepts(dx * step, dy * step);
            } else {
                dirty |= self.navigate(dx, dy);
            }
        }

        // Esc 逐层退：插入关系线 → 关系线选中 → 聚焦栈 → 多选。与
        // `inspector::esc_step` / `outline::esc_step` 同一口径（越「外」的临时状态越先退）。
        //
        // 必须排在 Tab/Enter/Delete **之前**：插入态是「等一个点击」，用户按 Esc 想退出，
        // 不能让同一次按键先给选中节点造出一个子节点。
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            // 关系线以前不进这条链：选中一条线按 `Esc`，被清掉的是节点多选，线还亮着。
            match esc_plan(
                self.rel_link.is_some(),
                !self.relationship.is_empty(),
                self.focus.len(),
                !self.sel.is_empty(),
            ) {
                CanvasEsc::RelLink => {
                    dirty |= self.cancel_rel_link("esc");
                }
                CanvasEsc::Relationship => {
                    self.relationship.clear();
                    dirty = true;
                }
                CanvasEsc::Focus => dirty |= self.focus_pop(),
                CanvasEsc::Selection => dirty |= self.sel.clear(),
                CanvasEsc::Nothing => {}
            }
        }
        // 插入态只认 Ctrl+L / Esc / 一次点击：别的结构键不能在这里生效。
        if self.rel_link.is_some() {
            return dirty;
        }

        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            // 概念图里 `Tab` 绝不能造树子节点：命题网的层级只由关系线表达，多一个
            // `parent_id` 就等于在地图里埋了一棵看不见的树（大纲、搜索、导出都会歪）。
            // 改走「加概念 + 加边 + 立刻填链接短语」，与大纲 `Tab` 的概念图分支同源。
            dirty |= if self.concept_map() {
                self.add_concept_relative(AddKind::Child)
            } else {
                self.add_relative(AddKind::Child, viewport)
            };
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            // 命题网没有「同级」，`Enter` 与 `Tab` 同义，只差落点方向（右 / 下）。
            dirty |= if self.concept_map() {
                self.add_concept_relative(AddKind::Sibling)
            } else {
                self.add_relative(AddKind::Sibling, viewport)
            };
        }
        let delete = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
            || ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
        if delete {
            // 一次 Delete 只作用一道：关系线 → 注解 → 节点（M26）。三道同时非空时
            // 以前会一起删，按一次键丢掉关系线和整棵子树。
            let lane = delete_lane(
                !self.relationship.is_empty(),
                self.annotations.len(),
                self.sel.ids().len(),
            );
            let name = match lane {
                DeleteLane::Relationship => "relationship",
                DeleteLane::Annotation => "annotation",
                DeleteLane::Nodes => "nodes",
                DeleteLane::Nothing => "nothing",
            };
            crate::trace::event(&format!("delete_dispatch lane={name}"));
            match lane {
                DeleteLane::Relationship => dirty |= self.delete_selected_relationship(),
                DeleteLane::Annotation => dirty |= self.delete_annotation(),
                DeleteLane::Nodes => dirty |= self.delete_selected(),
                DeleteLane::Nothing => {}
            }
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2)) {
            if !self.relationship.is_empty() {
                dirty |= self.edit_relationship_label_action();
            } else {
                let target = self
                    .sel
                    .primary()
                    .or_else(|| self.scene.find(&self.session.doc.root_id));
                if let Some(idx) = target {
                    self.start_edit(idx, viewport);
                    dirty = true;
                }
            }
        }

        dirty
    }

    pub fn undo(&mut self) -> bool {
        let depth_before = self.session.doc.undo_depth();
        let Some(cs) = self.session.doc.undo() else {
            return false;
        };
        // 证据：撤销踩到了哪一条、栈里还剩几条。粒度问题（一条命令 vs 两条）
        // 光看截图像「盒子少了一个」，说不清是撤了节点还是只撤了文字。
        crate::trace::event(&format!(
            "undo label={} left={}",
            cs.label,
            self.session.doc.undo_depth()
        ));
        // 撤销把内容退回到上一代：让场景先试上一代那份账，别先白跑一遍当前代。
        self.scene_cache.prefer_previous();
        let undone_arrange = self.arrange_undo_depth == Some(depth_before);
        if undone_arrange {
            self.arrange_undo_depth = None;
            self.arrange_redo_fit = true;
        }
        self.rebuild();
        if undone_arrange {
            // 位置撤回来了，取景也 `fit` 回这套位置——不然用户看到「原来的散布 + 整理后的取景」。
            crate::trace::event("undo fit=1");
            self.request_fit();
        }
        self.refocus_canvas = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(cs) = self.session.doc.redo() else {
            return false;
        };
        crate::trace::event(&format!(
            "redo label={} left={}",
            cs.label,
            self.session.doc.redo_depth()
        ));
        self.scene_cache.prefer_previous();
        let redid_arrange = self.arrange_redo_fit;
        if redid_arrange {
            self.arrange_redo_fit = false;
            self.arrange_undo_depth = Some(self.session.doc.undo_depth());
        }
        self.rebuild();
        if redid_arrange {
            self.request_fit();
        }
        self.refocus_canvas = true;
        true
    }

    /// Tab = 子节点；Enter = 同级（根节点没有同级，退化为子节点）。作用于**主选**。
    /// 返回是否真的改了文档（没有主选/命令失败时为 `false`）。
    /// Tab = 子节点；Enter = 同级（根节点没有同级，退化为子节点）。作用于**主选**。
    /// 返回是否真的改了文档（没有主选/命令失败时为 `false`）。
    pub(crate) fn add_relative(&mut self, kind: AddKind, viewport: [f32; 2]) -> bool {
        let Some(idx) = self
            .sel
            .primary()
            .or_else(|| self.scene.find(&self.session.doc.root_id))
        else {
            return false;
        };
        let Some(sel_id) = self.scene.ids.get(idx as usize).cloned() else {
            return false;
        };
        let Some(new_id) = self.add_to(&sel_id, kind) else {
            return false;
        };
        if let Some(i) = self.scene.find(&new_id) {
            self.select_node_only(i, "add-relative");
            self.start_edit(i, viewport);
            // 记下「这枚是这次编辑建出来的」：`Esc` 取消时连它一起收回去。
            // 同时记下此刻的撤销栈深，取消时据此判断「栈顶还是不是建它的那条命令」。
            let depth = self.session.doc.undo_depth();
            if let Some(ed) = self.editor.as_mut() {
                ed.fresh = true;
                ed.fresh_depth = Some(depth);
            }
            self.reveal_node(i);
        }
        true
    }

    /// 大纲/面板入口：在指定节点下（或后面）加一个空节点，返回新节点 id。
    /// 不进入编辑器——大纲要就地编辑自己那一行，画布编辑框不该抢焦点。
    pub fn add_child_to(&mut self, id: &str) -> Option<String> {
        self.add_to(id, AddKind::Child)
    }

    pub fn add_sibling_to(&mut self, id: &str) -> Option<String> {
        self.add_to(id, AddKind::Sibling)
    }

    fn add_to(&mut self, anchor: &str, kind: AddKind) -> Option<String> {
        let anchor_node = self.session.doc.node(anchor)?;
        let (parent, index) = match kind {
            AddKind::Child => (anchor.to_string(), None),
            AddKind::Sibling => match anchor_node.parent_id.clone() {
                Some(p) => {
                    let pos = self
                        .session
                        .doc
                        .node(&p)
                        .and_then(|n| n.children.iter().position(|c| c == anchor))
                        .unwrap_or(0);
                    (p, Some(pos + 1))
                }
                None => (anchor.to_string(), None),
            },
        };

        let mut commands = Vec::new();
        if self.session.doc.node(&parent).is_some_and(|n| n.collapsed) {
            commands.push(Command::SetCollapsed {
                id: parent.clone(),
                collapsed: false,
            });
        }
        commands.push(Command::AddNode {
            parent,
            index,
            text: String::new(),
            style: None,
        });
        let command = if commands.len() > 1 {
            Command::Batch { commands }
        } else {
            commands.pop().expect("至少一条命令")
        };

        let cs = self.session.doc.apply(&command).ok()?;
        let before: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
        let new_id = cs
            .after
            .iter()
            .find(|n| !before.contains(n.id.as_str()))
            .map(|n| n.id.clone())?;
        self.rebuild();
        if let Some(i) = self.scene.find(&new_id) {
            self.select_node_only(i, "add-to");
            self.reveal_node(i);
        }
        self.refocus_canvas = true;
        Some(new_id)
    }

    /// 大纲 Enter（光标在文字中间）：拆成两个同级。返回新节点 id，调用方接着编辑它。
    pub fn split_sibling_text(&mut self, id: &str, head: &str, tail: &str) -> Option<String> {
        let command = split_sibling_command(&self.session.doc, id, head, tail)?;
        self.apply_split(command)
    }

    /// 大纲 Ctrl+Enter（光标在文字中间）：后半段拆成子级。返回新节点 id。
    pub fn split_child_text(&mut self, id: &str, head: &str, tail: &str) -> Option<String> {
        let command = split_child_command(&self.session.doc, id, head, tail)?;
        self.apply_split(command)
    }

    fn apply_split(&mut self, command: Command) -> Option<String> {
        let cs = self.session.doc.apply(&command).ok()?;
        let before: HashSet<&str> = cs.before.iter().map(|n| n.id.as_str()).collect();
        let new_id = cs
            .after
            .iter()
            .find(|n| !before.contains(n.id.as_str()))
            .map(|n| n.id.clone())?;
        self.rebuild();
        if let Some(i) = self.scene.find(&new_id) {
            self.select_node_only(i, "split");
            self.reveal_node(i);
        }
        self.refocus_canvas = true;
        Some(new_id)
    }

    /// 大纲 Backspace（光标在开头）：把文字并进上一同级。返回 (上一兄弟 id, 拼接处字符位)。
    pub fn merge_into_previous(&mut self, id: &str, text: &str) -> Option<(String, usize)> {
        let (command, prev, caret) = merge_prev_command(&self.session.doc, id, text)?;
        self.session.doc.apply(&command).ok()?;
        self.rebuild();
        if let Some(i) = self.scene.find(&prev) {
            self.select_node_only(i, "merge-prev");
            self.reveal_node(i);
        }
        self.refocus_canvas = true;
        Some((prev, caret))
    }

    /// 大纲 Tab：成为前一个兄弟的末子。返回是否真的改了结构。
    pub fn indent_node(&mut self, id: &str) -> bool {
        if !self.apply_structural(demote_command(&self.session.doc, id)) {
            return false;
        }
        if let Some(i) = self.scene.find(id) {
            self.select_node_only(i, "indent");
        }
        true
    }

    /// 大纲 Shift+Tab：升到父级后面。返回是否真的改了结构。
    pub fn outdent_node(&mut self, id: &str) -> bool {
        if !self.apply_structural(promote_command(&self.session.doc, id)) {
            return false;
        }
        if let Some(i) = self.scene.find(id) {
            self.select_node_only(i, "outdent");
        }
        true
    }

    /// 大纲 Delete：只删指定节点（连带子树），删完选择交回父级。
    pub fn delete_node(&mut self, id: &str) -> bool {
        let Some(i) = self.scene.find(id) else {
            return false;
        };
        if id == self.session.doc.root_id {
            return false;
        }
        self.select_node_only(i, "outline-delete");
        self.delete_selected()
    }

    /// 大纲 ↑/↓ 重排：在兄弟里上移/下移一格。
    pub fn move_node_step(&mut self, id: &str, delta: i32) -> bool {
        let command = move_command(&self.session.doc, id, delta);
        if !self.apply_structural(command) {
            return false;
        }
        if let Some(i) = self.scene.find(id) {
            self.select_node_only(i, "outline-move");
        }
        true
    }

    /// 多选缩进：一条 Batch、一次撤销。
    pub fn indent_many(&mut self, ids: &[String]) -> bool {
        self.apply_batch_structure(ids, BatchOp::Indent)
    }

    /// 多选升级：一条 Batch、一次撤销。
    pub fn outdent_many(&mut self, ids: &[String]) -> bool {
        self.apply_batch_structure(ids, BatchOp::Outdent)
    }

    /// 多选上/下移：整组按方向移动一格，组内顺序不变。
    pub fn move_step_many(&mut self, ids: &[String], delta: i32) -> bool {
        let op = if delta < 0 {
            BatchOp::MoveUp
        } else {
            BatchOp::MoveDown
        };
        self.apply_batch_structure(ids, op)
    }

    fn apply_batch_structure(&mut self, ids: &[String], op: BatchOp) -> bool {
        let Some(command) = batch_structure_command(&self.session.doc, ids, op) else {
            if crate::trace::is_enabled() {
                crate::trace::event(&format!(
                    "structure_batch op={op:?} n={} command=none",
                    ids.len()
                ));
            }
            return false;
        };
        let primary = self.primary_id();
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        self.reselect_surviving(ids, primary.as_deref());
        // 整组移动也要跟住主选：与单选路径、折叠、方向键遍历同一套「别让选中的东西走丢」。
        if let Some(id) = primary
            .as_deref()
            .or_else(|| ids.first().map(String::as_str))
        {
            self.reveal_id(id);
        }
        self.refocus_canvas = true;
        true
    }

    /// 结构操作后保留整组选择；主选还在组里就继续当主选。
    fn reselect_surviving(&mut self, ids: &[String], primary: Option<&str>) {
        let mut indices: Vec<u32> = Vec::new();
        for id in ids {
            if let Some(i) = self.scene.find(id) {
                if !indices.contains(&i) {
                    indices.push(i);
                }
            }
        }
        if let Some(p) = primary.and_then(|p| self.scene.find(p)) {
            indices.retain(|&i| i != p);
            indices.push(p);
        }
        self.sel.replace(indices);
    }

    /// 多选删除：只删结构根；删完选择先退回父级链，
    /// 大纲层会按可见顺序把焦点交给删除区之后的第一行。
    pub fn delete_many(&mut self, ids: &[String]) -> bool {
        let root = self.session.doc.root_id.clone();
        let roots: Vec<String> = selected_roots(&self.session.doc, ids)
            .into_iter()
            .filter(|id| id != &root)
            .collect();
        if roots.is_empty() {
            return false;
        }
        let primary = self.primary_id();
        let mut chain: Vec<String> = Vec::new();
        let mut cur = primary
            .as_deref()
            .and_then(|id| self.session.doc.node(id))
            .and_then(|n| n.parent_id.clone());
        while let Some(p) = cur {
            cur = self.session.doc.node(&p).and_then(|n| n.parent_id.clone());
            chain.push(p);
        }
        if self
            .session
            .doc
            .apply(&Command::DeleteNodes { ids: roots })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        self.sel.replace(
            chain
                .iter()
                .find_map(|p| self.scene.find(p))
                .into_iter()
                .collect(),
        );
        self.refocus_canvas = true;
        true
    }

    /// 多选任务：同一状态一次写给整组；一条 Batch 一次撤销。
    pub fn set_task_many(&mut self, ids: &[String], task: Option<mw_core::TaskState>) -> bool {
        let commands: Vec<Command> = ids
            .iter()
            .filter(|id| self.session.doc.node(id).is_some())
            .map(|id| Command::SetTask {
                id: id.clone(),
                task,
            })
            .collect();
        if commands.is_empty() {
            return false;
        }
        let primary = self.primary_id();
        if self
            .session
            .doc
            .apply(&Command::Batch { commands })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        self.reselect_surviving(ids, primary.as_deref());
        self.refocus_canvas = true;
        true
    }

    /// 多选拖拽落点：整组按可见顺序插到同一父级；一条 Batch 一次撤销。
    pub fn move_many(&mut self, ids: &[String], new_parent: &str, index: Option<usize>) -> bool {
        let Some(command) = move_many_command(&self.session.doc, ids, new_parent, index) else {
            return false;
        };
        let primary = self.primary_id();
        if self.session.doc.apply(&command).is_err() {
            return false;
        }
        self.rebuild();
        self.reselect_surviving(ids, primary.as_deref());
        self.refocus_canvas = true;
        true
    }

    /// 松手改结构：落点合法就发一条（或一批）`MoveNode`，非法/原地不动则什么也不做。
    ///
    /// 返回是否真的改了文档——只有它变了才需要重排与重绘。
    /// 松手改结构：落点合法就发一条（或一批）`MoveNode`，非法/原地不动则什么也不做。
    ///
    /// 返回是否真的改了文档——只有它变了才需要重排与重绘。
    pub(crate) fn commit_move(&mut self, m: &Moving, pointed_inside: bool) -> bool {
        // 概念图：拖拽只落 `pos`，不参与「挂成子节点」那一套（`docs/30` §5.5）。
        if self.concept_map() {
            return self.commit_concept_move(m);
        }
        let zoom = self.camera.zoom.max(1e-4);
        let (dx, dy) = (m.offset.x / zoom, m.offset.y / zoom);
        let Some(dt) = m.drop else {
            // 松在空白处 = 分离为自由主题。但只有指针还落在画布里才算：
            // 把节点拖到面板上松手是一次放弃，不该把它从树上摘下来。
            if !pointed_inside {
                return false;
            }
            let dropped: Vec<(String, mw_core::Rect)> = m
                .roots
                .iter()
                .filter_map(|&i| {
                    self.scene
                        .ids
                        .get(i as usize)
                        .cloned()
                        .map(|id| (id, self.scene.rect(i)))
                })
                .collect();
            // Ctrl 还在按着：复制一份放到空白处，原件留在原地。
            if m.copy {
                let Some((cmd, tops)) = copy_floating_command(&self.session.doc, &dropped, dx, dy)
                else {
                    return false;
                };
                if self.session.doc.apply(&cmd).is_err() {
                    return false;
                }
                self.rebuild();
                self.sel.replace(restore_selection(
                    &tops,
                    &self.scene,
                    &self.session.doc.root_id,
                ));
                self.refocus_canvas = true;
                return true;
            }
            let Some(cmd) =
                detach_on_blank_command(&self.session.doc, &dropped, pointed_inside, dx, dy)
            else {
                return false;
            };
            if self.session.doc.apply(&cmd).is_err() {
                return false;
            }
            self.rebuild();
            let ids: Vec<String> = dropped.into_iter().map(|(id, _)| id).collect();
            self.sel.replace(restore_selection(
                &ids,
                &self.scene,
                &self.session.doc.root_id,
            ));
            self.refocus_canvas = true;
            return true;
        };
        // `dt.illegal` 只是上一帧的显示判定；`plan_moves` 会按当前 roots 再守一遍。
        let moves = mw_ui::drag::plan_moves(&m.tree, &m.roots, dt);
        if m.copy {
            // Ctrl+拖拽：复制一份到落点，原件不动（XMind 的 Duplicate 拖拽）。
            let roots: Vec<String> = moves
                .iter()
                .filter_map(|mv| self.scene.ids.get(mv.node as usize).cloned())
                .collect();
            let targets: Vec<(String, Option<usize>)> = moves
                .iter()
                .filter_map(|mv| {
                    self.scene
                        .ids
                        .get(mv.parent as usize)
                        .cloned()
                        .map(|p| (p, mv.index))
                })
                .collect();
            let Some((cmd, tops)) = copy_into_command(&self.session.doc, &roots, &targets) else {
                return false;
            };
            if self.session.doc.apply(&cmd).is_err() {
                return false;
            }
            self.rebuild();
            // 复制完选中的是**副本**，用户下一步多半是接着摆它们。
            self.sel.replace(restore_selection(
                &tops,
                &self.scene,
                &self.session.doc.root_id,
            ));
            self.refocus_canvas = true;
            return true;
        }
        let Some(cmd) = mw_ui::drag::move_commands(&self.scene, &moves) else {
            return false;
        };
        // id 必须在 apply 之前取：改完树，场景索引会整体漂移。
        let ids: Vec<String> = m
            .roots
            .iter()
            .filter_map(|&i| self.scene.ids.get(i as usize).cloned())
            .collect();
        if self.session.doc.apply(&cmd).is_err() {
            return false;
        }
        self.rebuild();
        // 拖完仍然选着被拖的那些节点：用户下一步多半是接着调它们。
        self.sel.replace(restore_selection(
            &ids,
            &self.scene,
            &self.session.doc.root_id,
        ));
        self.refocus_canvas = true;
        true
    }

    /// 进入/退出「连线」态（`Ctrl+L` 与右键菜单共用）。
    ///
    /// 只做视图态：不碰文档、不入撤销栈（真正建边是在落点上发一条 `AddRelationship`）。
    /// 概念图与思维导图共用这个键，但语义不同（`docs/30` §5.4.2）：
    ///
    /// - 概念图选中**两个**概念：直接建一条命题并立刻进短语编辑（CmapTools 的「连起来」）；
    /// - 概念图选中**一个**概念：进连线态，下一次点击另一个概念落地；
    /// - 思维导图：永远进连线态（父子结构由树决定，没有「两颗直连」这回事）。
    ///
    /// 返回是否改变了视图（需要重绘）。
    pub fn begin_relationship_action(&mut self) -> bool {
        if self.cancel_rel_link("toggle") {
            return true;
        }
        if self.editor.is_some() || self.summary_edit.is_some() || self.rel_edit.is_some() {
            return false;
        }
        if self.concept_map() {
            return self.begin_concept_link();
        }
        let notice = match self.sel.len() {
            1 => {
                let Some(id) = self
                    .sel
                    .primary()
                    .and_then(|i| self.scene.ids.get(i as usize))
                    .cloned()
                else {
                    return false;
                };
                self.relationship.clear();
                self.annotations.clear();
                self.rel_link = Some(id.clone());
                self.refocus_canvas = true;
                crate::trace::event(&format!("rel_link_mode source={id}"));
                ("点击另一个主题建立关系线 · Esc 取消".to_string(), false)
            }
            0 => ("先选中一个主题，再插入关系线".to_string(), true),
            _ => ("插入关系线只能从一个主题出发".to_string(), true),
        };
        self.notice = Some(notice);
        self.sel.len() == 1
    }

    /// 概念图的 `Ctrl+L` / 菜单「连接」入口（`docs/30` §5.4.2）。
    ///
    /// 选中两颗 → 直接连（源 = 场景顺序靠前的那颗，保证同一份文档两次操作结果一致）；
    /// 选中一颗 → 进连线态；其余情况只给提示，不改任何状态。
    fn begin_concept_link(&mut self) -> bool {
        let ids = self.selected_concept_ids();
        match crate::canvas::concept_link_intent(ids.len()) {
            crate::canvas::ConceptLinkIntent::Direct => {
                crate::trace::event(&format!(
                    "concept_link_direct from={} to={}",
                    ids[0], ids[1]
                ));
                self.connect_concepts(ids[0].clone(), ids[1].clone())
            }
            crate::canvas::ConceptLinkIntent::Arm => {
                let id = ids[0].clone();
                self.relationship.clear();
                self.annotations.clear();
                self.rel_link = Some(id.clone());
                self.refocus_canvas = true;
                self.notice = Some(("点击另一个概念建立连接 · Esc 取消".to_string(), false));
                crate::trace::event(&format!("rel_link_mode source={id} kind=concept"));
                true
            }
            crate::canvas::ConceptLinkIntent::NeedSelection => {
                self.notice = Some(("先选中一个概念，再连线".to_string(), true));
                crate::trace::event("concept_link_failed reason=no-selection");
                true
            }
            crate::canvas::ConceptLinkIntent::TooMany => {
                self.notice = Some(("连线只连接两个概念".to_string(), true));
                crate::trace::event("concept_link_failed reason=too-many");
                true
            }
        }
    }

    /// 在两个概念之间建一条命题，并立刻立起短语编辑框（空短语整条撤掉，§5.9）。
    ///
    /// 失败不静默：core 的守卫文案（自连 / 已存在）直接进提示条。
    pub(crate) fn connect_concepts(&mut self, from: String, to: String) -> bool {
        if from == to {
            self.notice = Some(("不能把概念连接到自身".to_string(), true));
            crate::trace::event("concept_link_failed reason=self");
            return true;
        }
        if let Err(err) = self.session.doc.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.clone(),
            label: None,
        }) {
            crate::trace::event(&format!(
                "concept_link_failed from={from} to={to} error={err}"
            ));
            self.notice = Some((err.to_string(), true));
            return true;
        }
        crate::trace::event(&format!("concept_link_created from={from} to={to}"));
        self.finish_new_link(from, to)
    }

    /// 把当前选中的概念建成一个分组（CM-18）。成员 < 2 只给提示、不发命令。
    pub(crate) fn create_group_from_selection(&mut self) -> bool {
        let ids = self.selected_concept_ids();
        if ids.len() < 2 {
            self.notice = Some(("先选中至少两个概念再分组".to_string(), true));
            crate::trace::event("group_create_reject reason=few");
            return true;
        }
        let label = format!("分组 {}", self.session.doc.groups.len() + 1);
        crate::trace::event(&format!("group_create members={}", ids.len()));
        self.apply_command(Command::CreateGroup {
            members: ids,
            label,
        })
    }

    /// 视觉验收（CM-18）：按概念文字建一个分组（`MW_CONCEPT_GROUP="A|B"`）。
    ///
    /// 走的是真实的「选中 → 创建分组」两步，所以基线锁的就是用户能走出来的路径。
    pub fn create_group_by_text(&mut self, members: &[&str]) -> bool {
        let ids: Vec<String> = members
            .iter()
            .filter_map(|t| self.node_id_by_text(t))
            .collect();
        if ids.len() < 2 {
            return false;
        }
        self.select_many(&ids, &ids[0].clone());
        self.create_group_from_selection()
    }

    /// 解组：只删框，概念一个不删（CM-18）。
    pub(crate) fn delete_context_group(&mut self) -> bool {
        let Some(id) = self.context_group.clone() else {
            return false;
        };
        crate::trace::event(&format!("group_delete {id}"));
        self.apply_command(Command::DeleteGroup { id })
    }

    /// 改右键那个分组的框色（CM-18）。
    pub(crate) fn set_context_group_color(&mut self, color: Option<&str>) -> bool {
        let Some(id) = self.context_group.clone() else {
            return false;
        };
        self.apply_command(Command::SetGroupColor {
            id,
            color: color.map(|c| c.to_string()),
        })
    }

    /// 选中右键那个分组的全部概念（CM-18）。
    pub(crate) fn select_context_group_members(&mut self) -> bool {
        let Some(id) = self.context_group.clone() else {
            return false;
        };
        let Some(g) = self.session.doc.groups.iter().find(|g| g.id == id) else {
            return false;
        };
        let ids: Vec<String> = g
            .members
            .iter()
            .filter(|m| self.scene.find(m).is_some())
            .cloned()
            .collect();
        let Some(first) = ids.first().cloned() else {
            return false;
        };
        self.select_many(&ids, &first);
        self.refocus_canvas = true;
        true
    }

    /// 当前选中的关系线是不是虚线（右键菜单「线型」打勾用）。
    pub(crate) fn selected_relationship_dashed(&self) -> Option<bool> {
        let sel = self.relationship.primary()?;
        self.session
            .doc
            .node(&sel.from)
            .and_then(|n| n.relationships.iter().find(|r| r.id == sel.id))
            .map(|r| r.dashed)
    }

    /// 改当前选中关系线的线型（`docs/30` §5.4.3，CM-16）。一条命令、一次撤销。
    pub(crate) fn set_selected_relationship_dashed(&mut self, dashed: bool) -> bool {
        let rels = self.relationship.as_slice().to_vec();
        if rels.is_empty() {
            return false;
        }
        let commands: Vec<Command> = rels
            .iter()
            .map(|r| Command::SetRelationshipDashed {
                from: r.from.clone(),
                id: r.id.clone(),
                dashed,
            })
            .collect();
        let cmd = match commands.len() {
            1 => commands.into_iter().next().unwrap(),
            _ => Command::Batch { commands },
        };
        self.apply_command(cmd)
    }

    /// 反向当前选中的关系线（`docs/30` §5.4.2）。一条命令、一次撤销。
    ///
    /// 反向后这条边换了所属节点（`from → to` 变成 `to → from`），所以选中态必须跟着挪，
    /// 否则检查器还指着旧的 `(from, id)`，用户会看到「选中的线不见了」。
    pub(crate) fn reverse_selected_relationship(&mut self) -> bool {
        let rels = self.relationship.as_slice().to_vec();
        if rels.is_empty() {
            return false;
        }
        let commands: Vec<Command> = rels
            .iter()
            .map(|r| Command::ReverseRelationship {
                from: r.from.clone(),
                id: r.id.clone(),
            })
            .collect();
        let cmd = match commands.len() {
            1 => commands.into_iter().next().unwrap(),
            _ => Command::Batch { commands },
        };
        if let Err(err) = self.session.doc.apply(&cmd) {
            crate::trace::event(&format!("rel_reverse_failed error={err}"));
            self.notice = Some((err.to_string(), true));
            return true;
        }
        for r in &rels {
            crate::trace::event(&format!("rel_reverse from={} to={}", r.from, r.to));
        }
        self.rebuild();
        // 反向后每条边换了所属节点（`from → to` 变成 `to → from`），选中集要跟着搬，
        // 否则检查器还指着旧的 `(from, id)`，用户会看到「选中的线不见了」。
        let flipped: Vec<RelationshipSel> = rels
            .iter()
            .map(|r| RelationshipSel {
                from: r.to.clone(),
                id: r.id.clone(),
                to: r.from.clone(),
            })
            .collect();
        self.relationship.clear();
        for f in flipped {
            let alive = self
                .scene
                .relationships
                .iter()
                .any(|r| r.id == f.id && self.scene.ids[r.from as usize] == f.from);
            if alive {
                self.relationship.toggle(f);
            }
        }
        self.refocus_canvas = true;
        true
    }

    /// 键盘加概要/边界（`Ctrl+B` / `Ctrl+]`），与右键菜单同名项共用动作。
    ///
    /// 菜单靠禁用态表达「现在不能加」，键盘没有禁用态，两条失败路径必须说清楚：
    /// 没选中、选了根（没有同一父级可圈）。概念图没有注解语义，不抢这两个键。
    pub(crate) fn add_annotation_from_keyboard(&mut self, add: AnnotationAdd) -> bool {
        if self.concept_map() || self.rel_link.is_some() {
            return false;
        }
        let kind = match add {
            AnnotationAdd::Boundary => "boundary",
            AnnotationAdd::Summary => "summary",
        };
        let Some(error) = annotation_shortcut_error(
            &self.session.doc,
            &self.selected_ids(),
            self.primary_id().as_deref(),
        ) else {
            return match add {
                AnnotationAdd::Boundary => self.add_boundary_action(),
                AnnotationAdd::Summary => self.add_summary_action(),
            };
        };
        let reason = match error {
            AnnotationShortcutError::NoSelection => "no-selection",
            AnnotationShortcutError::NoParent => "no-parent",
        };
        crate::trace::event(&format!(
            "annotation_add_failed kind={kind} reason={reason}"
        ));
        let text = match (add, error) {
            (AnnotationAdd::Boundary, AnnotationShortcutError::NoSelection) => {
                "先选中主题，再添加边界"
            }
            (AnnotationAdd::Summary, AnnotationShortcutError::NoSelection) => {
                "先选中主题，再添加概要"
            }
            (AnnotationAdd::Boundary, AnnotationShortcutError::NoParent) => {
                "边界只能圈同一父级下的主题"
            }
            (AnnotationAdd::Summary, AnnotationShortcutError::NoParent) => {
                "概要只能覆盖同一父级下的主题"
            }
        };
        self.notice = Some((text.to_string(), true));
        true
    }

    /// 退出插入态（`Esc` / 再按 `Ctrl+L` / 点了源主题或空白）。
    pub(crate) fn cancel_rel_link(&mut self, reason: &str) -> bool {
        let Some(src) = self.rel_link.take() else {
            return false;
        };
        // 进入插入态时那条「点击另一个主题…」的提示不能留在屏幕上：
        // 已经退出了，再写着指令就是骗人。
        self.notice = None;
        crate::trace::event(&format!("rel_link_cancel reason={reason} source={src}"));
        true
    }

    /// 插入态的落点：一条 `AddRelationship`，随后选中新线（检查器直接可改）。
    ///
    /// 失败（思维导图同一对主题已有线）不静默：提示条说明原因，插入态照常退出。
    pub(crate) fn commit_relationship_link(&mut self, to: &str) -> bool {
        let Some(from) = self.rel_link.take() else {
            return false;
        };
        if let Err(err) = self.session.doc.apply(&Command::AddRelationship {
            from: from.clone(),
            to: to.to_string(),
            label: None,
        }) {
            crate::trace::event(&format!("rel_link_failed from={from} to={to} error={err}"));
            self.notice = Some((err.to_string(), true));
            return true;
        }
        // 概念图：连线态落地的是一条**命题**，落完立刻写短语（与拖锚点同一条收尾，
        // `docs/30` §5.4.2）。思维导图仍只选中新线，交给检查器改。
        if self.concept_map() {
            crate::trace::event(&format!(
                "rel_link_created from={from} to={to} kind=concept"
            ));
            return self.finish_new_link(from, to.to_string());
        }
        let new_rel = self
            .session
            .doc
            .node(&from)
            .and_then(|n| n.relationships.last())
            .map(|r| r.id.clone());
        self.rebuild();
        self.refocus_canvas = true;
        if let Some(id) = new_rel {
            crate::trace::event(&format!("rel_link_created from={from} to={to} id={id}"));
            crate::trace::event(&format!("select relationship {from} {id}"));
            self.notice = Some(("已插入关系线 · Ctrl+Z 撤销".to_string(), false));
            self.select_relationship(&from, &id);
        }
        true
    }

    /// 新建一颗**独立概念**（不带边，`docs/33` §5）。
    ///
    /// 与「新建概念并连线」（`AddLinkedConcept`）的差别是语义：这颗是浮在画布上的
    /// 孤立概念，想连谁再自己拖——CmapTools 的「New Concept」就是这样。
    /// 落点在视口中央，并做落点避让（`docs/33` §2 B3）：不许一落地就压在别人身上。
    pub fn add_free_concept(&mut self) -> Option<String> {
        if !self.concept_map() {
            return None;
        }
        let (w, h) = (mw_core::scene::CONCEPT_MIN_W, mw_core::scene::CONCEPT_H1);
        let vp = self.last_viewport;
        let center = self
            .camera
            .screen_to_world(Point::new(vp[0] * 0.5, vp[1] * 0.5), vp);
        let (x, y) = self.free_concept_spot((center.x - w * 0.5, center.y - h * 0.5), (w, h), &[]);
        let id = mw_core::new_id();
        let mut node = mw_core::Node::new(id.clone(), None, CONCEPT_PLACEHOLDER.to_string());
        node.pos = Some(mw_core::model::Position { x, y });
        if self.apply_structural(Some(Command::InsertConcepts { nodes: vec![node] })) {
            crate::trace::event(&format!("concept_free_new id={id} at={x:.0},{y:.0}"));
            Some(id)
        } else {
            None
        }
    }

    /// 落点避让（`docs/33` §2 B3）：把一颗概念从 `want` 挪到最近的不重叠位置。
    ///
    /// 既有概念**一个像素都不动**——那可能是用户精心摆好的；`skip` 里的是这次
    /// 要一起动的（不算障碍）。绕不出去就原样返回，由调用方决定要不要提示。
    fn free_concept_spot(&self, want: (f32, f32), size: (f32, f32), skip: &[u32]) -> (f32, f32) {
        let others: Vec<((f32, f32), (f32, f32))> = self
            .scene
            .draw_order
            .iter()
            .copied()
            .filter(|i| !skip.contains(i))
            .map(|i| {
                let r = self.scene.rect(i);
                ((r.x, r.y), (r.w, r.h))
            })
            .collect();
        mw_core::concept_layout::resolve_overlap(want, size, &others)
    }

    /// 松手建关系线。松在空白处（或自己身上）什么也不做——一次误拖不该留下垃圾。
    pub(crate) fn commit_link(&mut self, l: &Link) -> bool {
        let Some(from_id) = self.scene.ids.get(l.from as usize).cloned() else {
            return false;
        };
        // CM-10 ①（对标 CmapTools）：拖到**空白处**松手 = 就地长出一个概念并连线。
        // 只有概念图这样：思维导图的父子由结构决定，画布落点没有语义。
        // 短语照旧留空并立刻立起编辑框，留空会把这条线连同新概念一起撤掉。
        let to_id = match l.to {
            Some(to) => match self.scene.ids.get(to as usize).cloned() {
                Some(id) => id,
                None => return false,
            },
            None => {
                if !self.concept_map() {
                    return false;
                }
                // 一步撤销：命令自己同时建概念与边（CM-10 ①），不走下面的 AddRelationship。
                let before = self.session.doc.doc_order();
                // 落点避让（`docs/33` §2 B3）：拖到别人头上就滑到最近的空位，
                // 免得新概念一落地就压在另一颗身上。新概念还没量出盒子，
                // 用 `measure_concept` 的下限估算。
                let (nx, ny) = self.free_concept_spot(
                    (l.at.x, l.at.y),
                    (mw_core::scene::CONCEPT_MIN_W, mw_core::scene::CONCEPT_H1),
                    &[],
                );
                let cmd = Command::AddLinkedConcept {
                    from: from_id.clone(),
                    text: super::CONCEPT_PLACEHOLDER.into(),
                    x: nx,
                    y: ny,
                };
                if !self.apply_structural(Some(cmd)) {
                    return false;
                }
                let Some(new_id) = self
                    .session
                    .doc
                    .doc_order()
                    .into_iter()
                    .find(|id| !before.contains(id))
                else {
                    return false;
                };
                crate::trace::event(&format!(
                    "link_new_concept from={} to={} at={:.0},{:.0}",
                    from_id, new_id, l.at.x, l.at.y
                ));
                // 边已由命令建好，直接收尾：不重复 AddRelationship。
                return self.finish_new_link(from_id, new_id);
            }
        };
        if self
            .session
            .doc
            .apply(&Command::AddRelationship {
                from: from_id.clone(),
                to: to_id.clone(),
                label: None,
            })
            .is_err()
        {
            return false;
        }
        self.finish_new_link(from_id, to_id)
    }

    /// 新线落盘后的收尾：找新边 id、重建场景、概念图立刻进短语编辑态。
    ///
    /// 两条入口共用：拖到已有概念（`AddRelationship`）与拖到空白（`AddLinkedConcept`）。
    fn finish_new_link(&mut self, from_id: String, to_id: String) -> bool {
        // 新线的 id 由命令生成：从源节点的关系表尾部取（`AddRelationship` 只往尾部 push）。
        let new_rel = self
            .session
            .doc
            .nodes
            .get(&from_id)
            .and_then(|n| n.relationships.last())
            .map(|r| r.id.clone());
        self.rebuild();
        self.refocus_canvas = true;
        // 概念图：拖完立刻进短语编辑态；留空取消会把这条线一起撤掉（`docs/30` §5.9）。
        if self.session.doc.meta.layout.kind == "conceptMap" {
            if let Some(id) = new_rel {
                let sel = RelationshipSel {
                    from: from_id,
                    id,
                    to: to_id,
                };
                let viewport = self.last_viewport;
                self.start_rel_edit(sel, viewport, true);
            }
        }
        true
    }

    /// 工具栏入口：加子主题（与 Tab 同源）。返回是否真的改了文档。
    pub fn add_child(&mut self) -> bool {
        if self.editor.is_some() {
            return false;
        }
        let viewport = self.last_viewport;
        self.add_relative(AddKind::Child, viewport)
    }

    /// 工具栏入口：加同级（与 Enter 同源）。
    pub fn add_sibling(&mut self) -> bool {
        if self.editor.is_some() {
            return false;
        }
        let viewport = self.last_viewport;
        self.add_relative(AddKind::Sibling, viewport)
    }

    pub fn can_undo(&self) -> bool {
        self.session.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.doc.can_redo()
    }

    /// 删除按钮可用性：至少有一个非根节点被选中。根节点永远不可删。
    ///
    /// 概念图例外（CM-05）：那里的 `root_id` 是「第一个概念」，删它由 `DeleteNodes`
    /// 重挂到别的概念上，所以唯一拦住删除的条件是「删完一个概念都不剩」。
    pub fn can_delete(&self) -> bool {
        if self.concept_map() {
            if self.concept_count() < 2 {
                return false;
            }
            return self.sel.ids().iter().any(|&i| {
                self.scene.ids.get(i as usize).is_some_and(|id| {
                    self.session
                        .doc
                        .node(id)
                        .is_some_and(|n| n.parent_id.is_none())
                })
            });
        }
        let root = self.session.doc.root_id.as_str();
        self.sel
            .ids()
            .iter()
            .filter_map(|&i| self.scene.ids.get(i as usize))
            .any(|id| id != root)
    }

    /// 文档里的概念总数（`parent_id == None` 的节点，含主概念）。
    fn concept_count(&self) -> usize {
        self.session
            .doc
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none())
            .count()
    }

    /// 行内编辑器打开时，撤销/加主题这类结构操作交给 TextEdit 自己处理。
    pub fn is_editing(&self) -> bool {
        self.editor.is_some()
    }

    /// 存盘/换文档前把编辑框里最后一段文字写进文档，否则它会随编辑器一起丢掉。
    pub fn commit_editing(&mut self) -> bool {
        if self.editor.is_none() {
            return false;
        }
        self.commit_edit();
        true
    }

    /// 网格与标尺：默认关。网格只画屏幕空间细线；标尺是顶部/左侧 18px 的刻度条。
    pub(crate) fn paint_grid_rulers(&self, ui: &egui::Ui, rect: egui::Rect, viewport: [f32; 2]) {
        let theme = &self.session.theme;
        let painter = ui.painter_at(rect);
        let step = self.grid_step.max(8.0);
        let zoom = self.camera.zoom.max(1e-4);
        let world_left = self.camera.center.x - viewport[0] * 0.5 / zoom;
        let world_top = self.camera.center.y - viewport[1] * 0.5 / zoom;
        let line = |a: u8| {
            let c = theme.ui.accent;
            egui::Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], a)
        };
        if self.show_grid && step * zoom >= 8.0 {
            let mut x = (world_left / step).floor() * step;
            let mut n = 0;
            while n < 400 {
                let s = self.camera.world_to_screen(Point::new(x, 0.0), viewport).x;
                if s > viewport[0] {
                    break;
                }
                if s >= 0.0 {
                    let sx = rect.min.x + s;
                    painter.vline(
                        sx,
                        rect.y_range(),
                        Stroke::new(1.0, line(if n % 5 == 0 { 26 } else { 12 })),
                    );
                }
                x += step;
                n += 1;
            }
            let mut y = (world_top / step).floor() * step;
            let mut n = 0;
            while n < 400 {
                let s = self.camera.world_to_screen(Point::new(0.0, y), viewport).y;
                if s > viewport[1] {
                    break;
                }
                if s >= 0.0 {
                    let sy = rect.min.y + s;
                    painter.hline(
                        rect.x_range(),
                        sy,
                        Stroke::new(1.0, line(if n % 5 == 0 { 26 } else { 12 })),
                    );
                }
                y += step;
                n += 1;
            }
        }
        if self.show_rulers {
            let strip = 18.0;
            let top = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), strip));
            let left = egui::Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + strip),
                Vec2::new(strip, rect.height() - strip),
            );
            painter.rect_filled(top, 0.0, color32(theme.ui.panel));
            painter.rect_filled(left, 0.0, color32(theme.ui.panel));
            let tick = |a: u8| {
                let c = theme.ui.border;
                egui::Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], a)
            };
            let mut x = (world_left / 100.0).floor() * 100.0;
            for _ in 0..200 {
                let s = self.camera.world_to_screen(Point::new(x, 0.0), viewport).x;
                if s > viewport[0] {
                    break;
                }
                if s >= 0.0 {
                    let sx = rect.min.x + s;
                    painter.vline(sx, top.y_range(), Stroke::new(1.0, tick(120)));
                }
                x += 100.0;
            }
            let mut y = (world_top / 100.0).floor() * 100.0;
            for _ in 0..200 {
                let s = self.camera.world_to_screen(Point::new(0.0, y), viewport).y;
                if s > viewport[1] {
                    break;
                }
                if s >= 0.0 {
                    let sy = rect.min.y + s;
                    painter.hline(left.x_range(), sy, Stroke::new(1.0, tick(120)));
                }
                y += 100.0;
            }
            painter.rect_stroke(
                top,
                0.0,
                Stroke::new(1.0, color32(theme.ui.border)),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                left,
                0.0,
                Stroke::new(1.0, color32(theme.ui.border)),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// 有卡片笔记的节点画一个小角标：右上角、屏幕空间恒定大小，不压节点文字。
    /// 悬浮显示标题/摘要，点击交给壳层打开阅读页。
    /// 有卡片笔记的节点画一个小角标：右上角、屏幕空间恒定大小，不压节点文字。
    /// 悬浮显示标题/摘要，点击交给壳层打开阅读页。
    pub(crate) fn paint_card_badges(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        viewport: [f32; 2],
    ) {
        debug_assert!(zorder::Plane::HoverBar.above(zorder::Plane::Badges));
        if self.camera.zoom < 0.28 || self.editor.is_some() || self.input_blocked || self.reading {
            return;
        }
        let theme = &self.session.theme;
        let doc = &self.session.doc;
        let radius = if self.camera.zoom < 0.55 { 3.5 } else { 6.5 };
        let mut clicked = None;
        for (idx, id) in self.scene.ids.iter().enumerate() {
            let Some(node) = doc.node(id) else {
                continue;
            };
            if node.card_id.is_none() {
                continue;
            }
            let Some(card) = doc.card_for_node(id) else {
                continue;
            };
            let geom = self.scene.nodes[idx].geom.rect;
            let tr = self
                .camera
                .world_to_screen(Point::new(geom.right(), geom.y), viewport);
            let center = card_badge_center(rect.min + Vec2::new(tr.x, tr.y), radius);
            if !zorder::badge_visible(center, rect, &self.overlay) {
                continue;
            }
            let hit = egui::Rect::from_center_size(center, Vec2::splat(radius * 2.0 + 6.0));
            let resp = ui.interact(
                hit,
                ui.scope_id().with(("mw-card-badge", id.as_str())),
                egui::Sense::click(),
            );
            let color = if resp.hovered() {
                color32(theme.ui.accent)
            } else {
                color32(theme.ui.border)
            };
            let painter = ui.painter_at(rect);
            painter.circle_filled(center, radius, color32(theme.ui.panel));
            painter.circle_stroke(center, radius, egui::Stroke::new(1.2, color));
            let w = radius * 0.85;
            let h = radius * 0.95;
            painter.rect_stroke(
                egui::Rect::from_center_size(center, Vec2::new(w, h)),
                1.0,
                egui::Stroke::new(1.0, color),
                egui::StrokeKind::Inside,
            );
            for k in [-0.22_f32, 0.05, 0.30] {
                painter.line_segment(
                    [
                        center + Vec2::new(-w * 0.28, h * k),
                        center + Vec2::new(w * 0.28, h * k),
                    ],
                    egui::Stroke::new(1.0, color),
                );
            }
            if resp.clicked() {
                clicked = Some(card.id.clone());
            }
            resp.on_hover_ui(|ui| {
                ui.set_max_width(240.0);
                ui.label(
                    egui::RichText::new(card.display_title())
                        .size(crate::chrome::Type::BODY)
                        .family(egui::FontFamily::Name(crate::fonts::BOLD.into())),
                );
                let excerpt = card.excerpt(80);
                if !excerpt.is_empty() {
                    ui.label(
                        egui::RichText::new(excerpt)
                            .size(crate::chrome::Type::SMALL)
                            .weak(),
                    );
                }
                ui.label(
                    egui::RichText::new("点击打开卡片")
                        .size(crate::chrome::Type::CAPTION)
                        .weak(),
                );
            });
        }
        if clicked.is_some() {
            self.card_open = clicked;
        }
    }

    /// 概念图里钉住的概念画一个位置受保护标记（右上角内侧微标）。
    pub(crate) fn paint_pinned_badges(&self, ui: &egui::Ui, rect: egui::Rect, viewport: [f32; 2]) {
        if !self.concept_map()
            || self.camera.zoom < 0.28
            || self.editor.is_some()
            || self.input_blocked
            || self.reading
        {
            return;
        }
        let theme = &self.session.theme;
        let doc = &self.session.doc;
        let radius = if self.camera.zoom < 0.55 { 3.5 } else { 5.5 };
        for (idx, id) in self.scene.ids.iter().enumerate() {
            let Some(node) = doc.node(id) else {
                continue;
            };
            if node.pinned != Some(true) {
                continue;
            }
            let geom = self.scene.nodes[idx].geom.rect;
            let tr = self
                .camera
                .world_to_screen(Point::new(geom.right(), geom.y), viewport);
            let center = rect.min + Vec2::new(tr.x - radius - 5.0, tr.y + radius + 5.0);
            if !zorder::badge_visible(center, rect, &self.overlay) {
                continue;
            }
            let painter = ui.painter_at(rect);
            painter.circle_filled(center, radius, color32(theme.ui.panel));
            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(1.2, color32(theme.ui.accent)),
            );
            painter.circle_filled(center, radius * 0.45, color32(theme.ui.accent));
        }
    }

    /// 分组框的名字（CM-18）：画在框的上沿外侧，屏幕空间 12px。
    ///
    /// 框本身在合成纹理里（wgpu 画的圆角虚线），字不能进纹理——它要跟主题字号走、
    /// 要能被系统字体回退，所以和其它覆盖层一样用 egui 画。
    pub(crate) fn paint_group_labels(&self, ui: &egui::Ui, origin: egui::Rect, viewport: [f32; 2]) {
        if self.scene.groups.is_empty() {
            return;
        }
        let painter = ui.painter_at(origin);
        for g in &self.scene.groups {
            let p = self
                .camera
                .world_to_screen(Point::new(g.label_at.x, g.label_at.y), viewport);
            let pos = egui::pos2(origin.min.x + p.x, origin.min.y + p.y);
            if !origin.contains(pos) {
                continue;
            }
            painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                &g.label,
                FontId::proportional(12.0),
                color32(g.color),
            );
        }
    }

    /// 关系线的强调：**选中** = 实色 + 3px；**悬停**（且没选中它）= 同色 45% + 2px。
    ///
    /// 这两样以前都没有：点中一条线，画布上一点变化都不发生，只有右侧检查器换了内容，
    /// 于是「我刚才点的是哪条」只能靠猜；悬停更是零暗示。CmapTools 里连接线一选中就亮，
    /// 这里补的是同一件事。
    ///
    /// 两个细节：
    /// - 路径来自 `Scene` 已算好的 `path`（世界坐标），按当前相机搬到屏幕，曲线采样成折线；
    /// - **碰到标签药丸要断开**：药丸在合成纹理里压在线上，强调线若照直画过去，
    ///   就成了给标签画删除线。断开用世界坐标做（药丸几何在世界空间）。
    ///
    /// 画哪一份 `Scene`：**这一帧真正画上去的那份**（过渡中就是插值帧），不是目标场景。
    /// 两条理由都在过渡期才看得见——新线正从端点「长」到终点时，照目标路径画重线，
    /// 屏幕上就多出一条比线本身还长的幽灵线；而淡入到一半的线 `rel.alpha` 只有 0.5，
    /// 强调若按目标场景的 1.0 算，就成了「半透明的线上盖一条满色实线」，
    /// 正是 [`emphasis_alpha`] 那段注释要挡的事。
    pub(crate) fn paint_relationship_emphasis(
        &self,
        ui: &egui::Ui,
        origin: egui::Rect,
        viewport: [f32; 2],
        scene: &Scene,
    ) {
        let accent = color32(self.session.theme.ui.accent);
        // 正在拖终点时不画强调：这一刻的焦点是预览线（1.6px 强调色），
        // 原来那条 3px 实线会让画面读成「拖出了一条平行线」。
        if self.relink_drag.is_some() {
            return;
        }
        // 三档里每一档都要求「有选中」或「有悬停」，三者皆无时整段跳过：
        // 上千条关系的大图上，白遍历一遍关系数组只为全部 continue 掉，是纯开销。
        if !emphasis_wanted(
            !self.relationship.is_empty(),
            self.hovered_rel.is_some(),
            self.hovered.is_some(),
        ) {
            return;
        }
        for rel in &scene.relationships {
            let Some(from) = scene.ids.get(rel.from as usize) else {
                continue;
            };
            let key = (from.clone(), rel.id.clone());
            // 三档强弱，眼睛要能分出「选中 / 直接指着它 / 指着它的某个概念」：
            // 选中最重（实色 3px），指着线次之（45% 2px），指着端点概念最轻（30% 1.8px）。
            // 第三档是读图的主路径——概念图里「这个概念连着哪些命题」是最常问的问题。
            let (width, alpha) = if self.relationship.contains(from, rel.id.as_str()) {
                (3.0, 1.0)
            } else if self.hovered_rel.as_ref() == Some(&key) {
                (2.0, 0.45)
            } else if self.hovered.is_some_and(|n| rel.from == n || rel.to == n) {
                (1.8, 0.30)
            } else {
                continue;
            };
            // 关系淡出到全透明时（alpha≈0）不必再加密/挖洞/三角化：算出来也是看不见的线，
            // 但采样、切段、建 mesh 样样都跑，一帧几十个点是纯白花。
            let emphasis = emphasis_alpha(alpha, rel.alpha);
            // 证据：强调线到底按多少不透明度画（档位 × 这一帧的 `rel.alpha`）。
            // 像素只能证明「亮了一点」，这一行才能证明「亮的是 0.15 而不是 0.30」——
            // 过渡中间帧的强调正是靠这个数落锁的。整组没变就不重复写。
            trace_rel_emphasis(ui, rel.id.as_str(), alpha, rel.alpha, emphasis, width);
            if !emphasis_visible(emphasis) {
                continue;
            }
            let hole = rel.label.as_ref().map(|_| label_pill_rect(rel));
            // 直线段只有两个点，`split_around` 切不动它的中段（挖掉一个端点就只剩一个点），
            // 药丸照样会被划穿。先按步长加密，直线和曲线就都走同一条挖洞路径。
            let pts = densify(&sample_path(&rel.path), EMPH_STEP);
            let runs = match hole {
                Some(h) => split_around(&pts, h),
                None => vec![pts],
            };
            let stroke = egui::Stroke::new(width, accent.gamma_multiply(emphasis));
            for run in runs {
                if run.len() < 2 {
                    continue;
                }
                let screen: Vec<Pos2> = run
                    .into_iter()
                    .map(|p| {
                        let s = self.camera.world_to_screen(p, viewport);
                        Pos2::new(origin.min.x + s.x, origin.min.y + s.y)
                    })
                    .collect();
                ui.painter().add(egui::Shape::line(screen, stroke));
            }
        }
    }

    /// 当前悬停节点的工具条（CSS px，屏幕空间）。编辑/拖拽/多选/浮层时不出现。
    pub(crate) fn hover_bar(&self, rect: egui::Rect, viewport: [f32; 2]) -> Option<HoverBar> {
        if self.editor.is_some()
            || self.drag.is_some()
            || self.moving.is_some()
            || self.link.is_some()
            || self.input_blocked
        {
            return None;
        }
        let idx = self.hovered?;
        let n = self.scene.nodes.get(idx as usize)?;
        let r = n.geom.rect;
        let a = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
        let b = self
            .camera
            .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
        let node_rect = egui::Rect::from_min_max(
            rect.min + Vec2::new(a.x, a.y),
            rect.min + Vec2::new(b.x, b.y),
        );
        let has_card = self
            .session
            .doc
            .node(&self.scene.ids[idx as usize])
            .is_some_and(|n| n.card_id.is_some());
        let (plus, more) = hover_bar_rects(node_rect, rect, has_card)?;
        Some(HoverBar {
            node: idx,
            node_rect,
            plus,
            more,
        })
    }

    /// 搜索命中的呼吸外环：前 700ms 两拍扩散淡出，之后转为常亮细环。
    /// 选择一变（点击别处/换搜索结果）就自然消失，不需要额外清理。
    /// 搜索命中的呼吸外环：前 700ms 两拍扩散淡出，之后转为常亮细环。
    /// 选择一变（点击别处/换搜索结果）就自然消失，不需要额外清理。
    /// 概念图的悬停提示（docs/30 §5.3.1）：只给**真被截断**的概念和卡片圆点，
    /// 同一目标停满 400ms 才出——鼠标划过整张图时不该一路闪提示。
    ///
    /// 位置钉在节点下边缘外 8px、最大宽 320：概念盒最宽 240，提示不会宽出一倍，
    /// 视线不用横跳。离屏/golden 下 `hovered` 恒为 `None`，这里自然什么都不画。
    pub(crate) fn paint_concept_tip(
        &mut self,
        ui: &egui::Ui,
        origin: egui::Rect,
        viewport: [f32; 2],
    ) {
        if !self.scene.kind.is_concept_map()
            || self.editor.is_some()
            || self.reading
            || self.drag.is_some()
        {
            self.tip = None;
            return;
        }
        let Some(n) = self.hovered else {
            self.tip = None;
            return;
        };
        let (Some(node), Some(id)) = (
            self.scene.nodes.get(n as usize),
            self.scene.ids.get(n as usize),
        ) else {
            self.tip = None;
            return;
        };
        let r = node.geom.rect;
        let s0 = self
            .camera
            .world_to_screen(crate::canvas::Point::new(r.x, r.y), viewport);
        let s1 = self
            .camera
            .world_to_screen(crate::canvas::Point::new(r.right(), r.bottom()), viewport);
        let sr = egui::Rect::from_min_max(
            Pos2::new(origin.min.x + s0.x, origin.min.y + s0.y),
            Pos2::new(origin.min.x + s1.x, origin.min.y + s1.y),
        );
        // 圆点是屏幕恒定 5px：用屏幕坐标判命中，缩放不改变手感。
        let dot = Pos2::new(
            sr.right() - mw_render::instances::CONCEPT_CARD_DOT_INSET,
            sr.bottom() - mw_render::instances::CONCEPT_CARD_DOT_INSET,
        );
        let on_dot = node.has_card
            && ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| p.distance(dot) <= 7.0);
        let text = if on_dot {
            self.session
                .doc
                .card_for_node(id)
                .map(|c| format!("已关联卡片：{}", c.title))
        } else if node.truncated {
            self.session.doc.node(id).map(|x| x.text.clone())
        } else {
            None
        };
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            self.tip = None;
            return;
        };
        let now = ui.input(|i| i.time);
        let key = Some((n, on_dot));
        if self.tip != key {
            self.tip = key;
            self.tip_at = now;
            return;
        }
        if now - self.tip_at < 0.40 {
            return;
        }
        egui::Area::new(egui::Id::unique("concept-tip"))
            .order(egui::Order::Tooltip)
            .fixed_pos(Pos2::new(sr.left(), sr.bottom() + 8.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(320.0);
                    ui.label(text);
                });
            });
    }
    pub(crate) fn paint_glow(&mut self, ui: &egui::Ui, rect: egui::Rect, viewport: [f32; 2]) {
        let Some(glow) = self.glow.as_ref() else {
            return;
        };
        let Some(idx) = self.scene.find(&glow.id) else {
            self.glow = None;
            return;
        };
        if !self.sel.contains(idx) {
            self.glow = None;
            return;
        }
        let node = &self.scene.nodes[idx as usize];
        let tl = self
            .camera
            .world_to_screen(Point::new(node.geom.rect.x, node.geom.rect.y), viewport);
        let br = self.camera.world_to_screen(
            Point::new(node.geom.rect.right(), node.geom.rect.bottom()),
            viewport,
        );
        let target = egui::Rect::from_min_max(
            rect.min + Vec2::new(tl.x, tl.y),
            rect.min + Vec2::new(br.x, br.y),
        );
        let elapsed = glow.start.elapsed().as_secs_f32();
        let (spread, alpha) = if self.reduce_motion {
            (4.0, 120.0)
        } else if elapsed < 0.7 {
            let phase = (elapsed / 0.35) % 1.0;
            ui.ctx().request_repaint();
            (4.0 + phase * 14.0, (1.0 - phase) * 230.0)
        } else {
            (4.0, 90.0)
        };
        let a = self.session.theme.ui.accent;
        ui.painter_at(rect).rect_stroke(
            target.expand(spread),
            NODE_RADIUS * self.camera.zoom + spread,
            egui::Stroke::new(
                2.0,
                Color32::from_rgba_unmultiplied(a.0[0], a.0[1], a.0[2], alpha as u8),
            ),
            egui::StrokeKind::Outside,
        );
    }

    /// 概念图落点重叠的 1px 告警边（`docs/30` §5.5）：亮 0.25s 就散，不写进文档、不进导出。
    ///
    /// 画在合成纹理之上：它是**交互反馈**，和落点虚线、橡皮筋同一层。
    pub(crate) fn paint_overlap_flash(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        viewport: [f32; 2],
    ) {
        let Some((ids, started)) = self.overlap_flash.as_ref() else {
            return;
        };
        if !overlap_flash_live(*started, self.now, self.flash_window) {
            self.overlap_flash = None;
            return;
        }
        let ids = ids.clone();
        let color = design::content::ContentColors::for_theme(&self.session.theme).warn;
        let stroke = egui::Stroke::new(
            design::stroke::HAIRLINE.max(1.0),
            Color32::from_rgba_unmultiplied(color.0[0], color.0[1], color.0[2], 235),
        );
        let painter = ui.painter_at(rect);
        let radius = NODE_RADIUS * self.camera.zoom;
        for id in &ids {
            let Some(idx) = self.scene.find(id) else {
                continue;
            };
            let r = self.scene.rect(idx);
            let tl = self.camera.world_to_screen(Point::new(r.x, r.y), viewport);
            let br = self
                .camera
                .world_to_screen(Point::new(r.right(), r.bottom()), viewport);
            let target = egui::Rect::from_min_max(
                rect.min + Vec2::new(tl.x, tl.y),
                rect.min + Vec2::new(br.x, br.y),
            );
            crate::trace::event(&format!("overlap_flash_rect id={id} target={:?}", target));
            painter.rect_stroke(
                target.expand(1.0),
                radius + 1.0,
                stroke,
                egui::StrokeKind::Outside,
            );
            // 诊断用：同一几何换成纯品红，用来区分「几何没画出来」与「颜色不对」。
            painter.rect_stroke(
                target.expand(3.0),
                radius + 3.0,
                egui::Stroke::new(1.0, Color32::from_rgb(255, 0, 255)),
                egui::StrokeKind::Outside,
            );
        }
        // 0.25s 的老化靠重绘推进：不主动要帧的话，静态画面会停在红边上。
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }

    /// 删除整组选中节点。根节点不可删——组里混进根时只跳过它，不让整次删除失败。
    ///
    /// 删完选谁：沿主选的祖先链往上找第一个**还活着**的节点（docs/02：删后落在父级）。
    /// 祖先链必须在 apply 之前取——删完树已经不通了。
    ///
    /// **带子节点的删除要先确认**：`DeleteNodes` 会连整片子树一起拿走，一次误按
    /// 就是几十个节点消失（虽然在撤销栈里，但用户得先知道发生了什么）。叶子节点
    /// 保持一键速删——把 `Delete` 变成两次点击会毁掉最高频的那条动线。
    pub fn delete_selected(&mut self) -> bool {
        // 确认框已经立着：再按 `Delete` 不该叠第二个、更不该直接删。
        if self.confirm_delete.is_some() {
            return false;
        }
        let Some(ids) = self.delete_ids() else {
            return false;
        };
        if let Some(count) = delete_confirm_plan(&self.session.doc, &ids) {
            crate::trace::event(&format!(
                "delete-confirm pending={count} selected={}",
                ids.len()
            ));
            self.confirm_delete = Some(DeleteConfirm { ids, count });
            self.refocus_canvas = true;
            // 画面上多了一个确认框：这一帧必须重画，否则要等下一次输入才看得见。
            return true;
        }
        self.delete_now(&ids)
    }

    /// 这一帧真正会进 `DeleteNodes` 的 id（跳过根节点）。`None` = 没有可删的。
    fn delete_ids(&self) -> Option<Vec<String>> {
        let root = self.session.doc.root_id.clone();
        // 概念图的主概念可以删（删除时把根重挂到别的概念上），树的根永远不行。
        let concept = self.concept_map();
        let ids: Vec<String> = self
            .sel
            .ids()
            .iter()
            .filter_map(|&i| self.scene.ids.get(i as usize).cloned())
            .filter(|id| concept || id != &root)
            .collect();
        (!ids.is_empty()).then_some(ids)
    }

    /// 真正下发删除：一条命令、一条撤销，删完选择落到父级。
    pub(crate) fn delete_now(&mut self, ids: &[String]) -> bool {
        if ids.is_empty() {
            return false;
        }
        let primary = self
            .sel
            .primary()
            .and_then(|i| self.scene.ids.get(i as usize).cloned());
        let mut chain: Vec<String> = Vec::new();
        let mut cur = primary
            .as_deref()
            .and_then(|id| self.session.doc.node(id))
            .and_then(|n| n.parent_id.clone());
        while let Some(p) = cur {
            cur = self.session.doc.node(&p).and_then(|n| n.parent_id.clone());
            chain.push(p);
        }
        if self
            .session
            .doc
            .apply(&Command::DeleteNodes { ids: ids.to_vec() })
            .is_err()
        {
            return false;
        }
        self.rebuild();
        let target = chain.iter().find_map(|p| self.scene.find(p));
        self.sel.replace(target.into_iter().collect());
        self.refocus_canvas = true;
        true
    }
}

/// 吃掉一个方向键，归一成单位向量（左/上为负）。
///
/// `consume_key` 的修饰键匹配会忽略多余的 `Shift`，所以 `Shift+方向` 也走这里；
/// 步长由调用方读 `modifiers.shift` 决定，别在这儿分叉——分叉会让「1px 还是 8px」
/// 这件事出现第二个真相源。
fn take_arrow(ui: &mut egui::Ui) -> Option<(f32, f32)> {
    const ARROWS: [(Key, (f32, f32)); 4] = [
        (Key::ArrowLeft, (-1.0, 0.0)),
        (Key::ArrowRight, (1.0, 0.0)),
        (Key::ArrowUp, (0.0, -1.0)),
        (Key::ArrowDown, (0.0, 1.0)),
    ];
    for (key, dir) in ARROWS {
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
            return Some(dir);
        }
    }
    None
}

/// 关系线标签药丸的世界坐标矩形（与合成器 `instances.rs` 里画药丸用的是同一份几何）。
///
/// 药丸高 `REL_LABEL_H`、宽取 `label_w` 与最小宽中较大者，中心在 `label_at`。
fn label_pill_rect(rel: &mw_core::scene::SceneRelationship) -> mw_core::geom::Rect {
    label_pill_rect_at(rel.label_at, rel.label_w)
}

/// `label_pill_rect` 的纯几何部分：能单测，不必造 `Scene`（那份要 GPU）。
pub(crate) fn label_pill_rect_at(at: mw_core::geom::Point, label_w: f32) -> mw_core::geom::Rect {
    let w = label_w.max(mw_render::instances::REL_LABEL_MIN_W);
    let h = mw_render::instances::REL_LABEL_H;
    // 外扩 1px：药丸边缘有抗锯齿，正好贴在边上会漏出一条线头。
    mw_core::geom::Rect::new(at.x - w * 0.5 - 1.0, at.y - h * 0.5 - 1.0, w + 2.0, h + 2.0)
}

/// 把世界坐标的折/曲线采样成折线。采样数按段固定（12 段），够平滑又不吃性能。
///
/// 单独拎出来是为了能单测：`Canvas` 要造合成器（GPU），`Camera` 是纯数据。
pub(crate) fn sample_path(path: &mw_core::geom::Path) -> Vec<mw_core::geom::Point> {
    const STEPS: usize = 12;
    let mut out = Vec::with_capacity(STEPS * path.segs.len() + 1);
    out.push(path.start);
    let mut cur = path.start;
    for seg in &path.segs {
        match *seg {
            mw_core::geom::PathSeg::Line { to } => out.push(to),
            mw_core::geom::PathSeg::Quad { ctrl, to } => {
                for i in 1..=STEPS {
                    let t = i as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    out.push(mw_core::geom::Point::new(
                        u * u * cur.x + 2.0 * u * t * ctrl.x + t * t * to.x,
                        u * u * cur.y + 2.0 * u * t * ctrl.y + t * t * to.y,
                    ));
                }
            }
            mw_core::geom::PathSeg::Cubic { c1, c2, to } => {
                for i in 1..=STEPS {
                    let t = i as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    let (u2, t2) = (u * u, t * t);
                    out.push(mw_core::geom::Point::new(
                        u2 * u * cur.x + 3.0 * u2 * t * c1.x + 3.0 * u * t2 * c2.x + t2 * t * to.x,
                        u2 * u * cur.y + 3.0 * u2 * t * c1.y + 3.0 * u * t2 * c2.y + t2 * t * to.y,
                    ));
                }
            }
        }
        cur = match *seg {
            mw_core::geom::PathSeg::Line { to }
            | mw_core::geom::PathSeg::Quad { to, .. }
            | mw_core::geom::PathSeg::Cubic { to, .. } => to,
        };
    }
    out
}

/// 把折线在 `hole` 内部的部分挖掉，返回若干段仍然在洞外的折线。
///
/// 洞内的点直接丢弃：采样点足够密，丢掉之后剩下的两段端点离洞边不到一个采样步长，
/// 肉眼看不出缺口；真的要精确，得求交点，那点收益不值得。
pub(crate) fn split_around(
    pts: &[mw_core::geom::Point],
    hole: mw_core::geom::Rect,
) -> Vec<Vec<mw_core::geom::Point>> {
    let mut runs: Vec<Vec<mw_core::geom::Point>> = Vec::new();
    let mut cur: Vec<mw_core::geom::Point> = Vec::new();
    for &p in pts {
        if hole.contains(p) {
            if cur.len() >= 2 {
                runs.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        } else {
            cur.push(p);
        }
    }
    if cur.len() >= 2 {
        runs.push(cur);
    }
    runs
}

/// 关系线强调的加密步长（世界单位）：远小于药丸尺寸，也远小于 3px 描边的可见误差。
const EMPH_STEP: f32 = 6.0;

/// 把折线按最大步长加密：相邻点距离超过 `step` 就插点。
///
/// 只为挖洞服务——没有它，一条直线段只有首末两点，标签药丸永远挖不掉。
pub(crate) fn densify(pts: &[mw_core::geom::Point], step: f32) -> Vec<mw_core::geom::Point> {
    let step = if step > 0.5 { step } else { 0.5 };
    let mut out: Vec<mw_core::geom::Point> = Vec::with_capacity(pts.len() * 2);
    let Some(&first) = pts.first() else {
        return out;
    };
    out.push(first);
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        let n = (len / step).ceil() as usize;
        for i in 1..n {
            let t = i as f32 / n as f32;
            out.push(mw_core::geom::Point::new(a.x + dx * t, a.y + dy * t));
        }
        out.push(b);
    }
    out
}

/// 强调线的最终不透明度 = 档位 × 关系自身的过渡透明度。
///
/// 概念图里新建/删除关系有淡入淡出（`SceneRelationship.alpha`）。强调若不乘它，
/// 一条正在淡入到 30% 的线会被画成满色实线，看着像有条幽灵线浮在图上面。
pub(crate) fn emphasis_alpha(tier: f32, rel_alpha: f32) -> f32 {
    (tier * rel_alpha.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// 强调线是否值得画：低于这个值肉眼已经看不见，画了只是白花采样与三角化。
///
/// 0.001 是「8 位色深下 1/255 ≈ 0.004」的一半，留够余量：真到 0.001 的线在任何主题下都读不出来。
pub(crate) fn emphasis_visible(alpha: f32) -> bool {
    alpha > 0.001
}

/// 证据：把「这一帧每条强调线按多少不透明度画」写进 `MW_TRACE`。
///
/// 像素只能证明「这条线亮了」，说不了它亮的是 0.15 还是 0.30——而过渡中间帧的强调
/// 恰恰只有这个数说得清。每条关系各占一行，值没变就不重复写（过渡每帧都在重绘）。
/// 没设 `MW_TRACE` 时整段不进。
fn trace_rel_emphasis(
    ui: &egui::Ui,
    id: &str,
    tier: f32,
    rel_alpha: f32,
    emphasis: f32,
    width: f32,
) {
    if !crate::trace::enabled() {
        return;
    }
    let sig =
        format!("id={id} tier={tier:.2} rel_alpha={rel_alpha:.3} alpha={emphasis:.3} w={width:.1}");
    let key = egui::Id::unique("mw_rel_emphasis").with(id);
    if ui.ctx().data(|d| d.get_temp::<String>(key)).as_deref() == Some(sig.as_str()) {
        return;
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, sig.clone()));
    crate::trace::event(&format!("rel_emphasis {sig}"));
}

/// 有没有任何东西能触发强调：选中一条关系、指着一条关系、或指着一个概念。
///
/// 三档强调分别要求这三种状态之一，所以三者皆无 ⇒ 这个函数画不出任何东西，可以整段跳过。
pub(crate) fn emphasis_wanted(
    has_selected: bool,
    has_hovered_rel: bool,
    has_hovered_node: bool,
) -> bool {
    has_selected || has_hovered_rel || has_hovered_node
}

// ─────────────────────── 本轮新增：交互的纯逻辑（都可单测） ───────────────────────

/// 从事件队列里摘一条「修饰键**完全一致**」的按键。
///
/// `InputState::consume_key` 的匹配是宽松的：它只校验「要求的修饰键按下了」，多按的
/// `Shift`/`Alt` 一概不管。编辑态因此必须走这里——否则 `Shift+Enter`（软换行）会被
/// `consume_key(Modifiers::NONE, Key::Enter)` 一并吃掉，变成「提交 + 新建同级」。
/// 取注解快捷键：只认**精确**的 `Ctrl+B` / `Ctrl+]`。`consume_key` 会忽略多余的
/// `Shift`，那样 `Ctrl+Shift+B` 也会建边界；这里用 `take_exact_key` 挡掉变体。
fn take_annotation_shortcut(ui: &egui::Ui) -> Option<AnnotationAdd> {
    [Key::B, Key::CloseBracket].into_iter().find_map(|key| {
        take_exact_key(ui, Modifiers::COMMAND, key)
            .then(|| annotation_add_for_key(Modifiers::COMMAND, key))
            .flatten()
    })
}

pub(crate) fn take_exact_key(ui: &egui::Ui, mods: Modifiers, key: Key) -> bool {
    ui.input_mut(|i| take_exact_key_from(&mut i.events, mods, key))
}

pub(crate) fn take_exact_key_from(
    events: &mut Vec<egui::Event>,
    mods: Modifiers,
    key: Key,
) -> bool {
    let idx = events.iter().position(|e| match e {
        egui::Event::Key {
            key: k,
            pressed: true,
            modifiers,
            ..
        } => *k == key && modifiers.matches_exact(mods),
        _ => false,
    });
    idx.is_some_and(|idx| {
        events.remove(idx);
        true
    })
}

/// 在光标（或选区）处插入一个软换行，返回新文本与新光标位（**按 `char` 数**）。
///
/// 有选区时整段替换，与 `TextEdit` 里按 `Shift+Enter` 的直觉一致；越界下标一律夹到末尾，
/// 全程按字符数换算，绝不切进 UTF-8 中间。
pub(crate) fn soft_newline_insert(text: &str, sel: Option<(usize, usize)>) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = match sel {
        Some((a, b)) => (a.min(b).min(chars.len()), a.max(b).min(chars.len())),
        None => (chars.len(), chars.len()),
    };
    let mut out = String::with_capacity(text.len() + 1);
    out.extend(chars[..start].iter());
    out.push('\n');
    out.extend(chars[end..].iter());
    (out, start + 1)
}

/// 非编辑态落字：直接进入编辑。IME 预编辑没有可显示的文字，先开一个空编辑框，
/// 让下一帧的 `TextEdit` 接管组合串（与大纲 `panels/outline.rs` 同一口径）。
pub(crate) enum TypeStart {
    Replace(String),
    Compose,
}

/// 只有「可打印、且不是空格」的一条文字才触发打字进编辑：空格在画布上另有归属
/// （平移/展开），吃掉它就等于把那个手势废掉。
pub(crate) fn type_char(text: &str) -> bool {
    !text.is_empty()
        && text != " "
        && !text.chars().any(|c| c.is_control())
        && !text.contains('\n')
        && !text.contains('\t')
}

/// 本帧的「打字进编辑」事件。IME 提交优先于 `Text`（两者可能同帧到）；预编辑不消费，
/// 留给下一帧的 `TextEdit` 继续组合。
pub(crate) fn take_type_start(events: &mut Vec<egui::Event>, mods: Modifiers) -> Option<TypeStart> {
    // 带 `Ctrl`/`Alt`/`Cmd` 的 `Text` 是快捷键路径上掉下来的，不是用户想改名字。
    if mods.ctrl || mods.alt || mods.command || mods.mac_cmd {
        return None;
    }
    let commit = events.iter().position(|e| match e {
        egui::Event::Ime(egui::ImeEvent::Commit(text)) => !text.is_empty(),
        _ => false,
    });
    if let Some(idx) = commit {
        if let egui::Event::Ime(egui::ImeEvent::Commit(text)) = events.remove(idx) {
            return Some(TypeStart::Replace(text));
        }
    }
    let typed = events.iter().position(|e| match e {
        egui::Event::Text(text) => type_char(text),
        _ => false,
    });
    if let Some(idx) = typed {
        if let egui::Event::Text(text) = events.remove(idx) {
            return Some(TypeStart::Replace(text));
        }
    }
    if events.iter().any(|e| match e {
        egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => !text.is_empty(),
        _ => false,
    }) {
        return Some(TypeStart::Compose);
    }
    None
}

/// 这批 id 删下去一共会消失几个节点，以及**要不要先确认**。
///
/// `None` = 不必确认（全是叶子）：把 `Delete` 变成两次点击会毁掉最高频的那条动线。
/// 有任何一个还带着子节点就要确认——`DeleteNodes` 连整片子树一起拿走，一次误按就是
/// 几十个节点消失。计数含整片子树并去重（父子同时被选中时，后代会被数两次）。
pub(crate) fn delete_confirm_plan(doc: &mw_core::Document, ids: &[String]) -> Option<usize> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut count = 0usize;
    let mut branching = false;
    for id in ids {
        for sub in doc.subtree_ids(id) {
            if !seen.insert(sub.clone()) {
                continue;
            }
            count += 1;
            if doc.node(&sub).is_some_and(|n| !n.children.is_empty()) {
                branching = true;
            }
        }
    }
    (branching && count > 0).then_some(count)
}

/// `Esc` 这一下该退哪一层。
pub(crate) enum CanvasEsc {
    /// 插入关系线（最外层：它只是「等一个点击」，取消不碰文档）。
    RelLink,
    Relationship,
    Focus,
    Selection,
    Nothing,
}

/// 取消一枚「这次编辑顺手建出来的」节点。
///
/// 栈顶仍是建它那条命令时（`fresh_depth` 与当前栈深相同），一次 `undo` 就把整个动作
/// 收回——否则栈里会多压一条 `DeleteNodes`，用户得按两下 `Ctrl+Z` 才回到建之前。
/// 深度对不上（中途做过别的动作）就退回「删掉这枚空盒子」，不去猜那是谁的栈顶。
///
/// 返回这次是不是走的一次撤销：`true` = 栈少了一条，`false` = 压了一条新命令。
pub(crate) fn cancel_fresh_node(
    doc: &mut mw_core::Document,
    id: &str,
    fresh_depth: Option<usize>,
) -> bool {
    if fresh_depth == Some(doc.undo_depth()) && doc.undo().is_some() {
        return true;
    }
    let _ = doc.apply(&Command::DeleteNodes {
        ids: vec![id.to_string()],
    });
    false
}

/// 全选的下标集合。主选排在**最后**：`Selection` 的最后一位是主选
/// （与 `select_all_concepts` 同一口径），方向键、检查器都读这一位。
/// `primary` 不在可见集合里时不强塞——它已经被折叠藏起来了。
pub(crate) fn select_all_indices(count: usize, primary: Option<u32>) -> Vec<u32> {
    let mut idxs: Vec<u32> = (0..count as u32).collect();
    if let Some(p) = primary.filter(|p| (*p as usize) < count) {
        idxs.retain(|&i| i != p);
        idxs.push(p);
    }
    idxs
}

/// `Esc` 逐层退：先退最「临时」的选中，最后才动选择本身。
///
/// 顺序与 `inspector::esc_step` / `outline::esc_step` 一致。关系线以前不进这条链，
/// 于是选中一条线按 `Esc`，被清掉的是节点多选、线还亮着。
pub(crate) fn esc_plan(
    has_rel_link: bool,
    has_relationship: bool,
    focus_depth: usize,
    has_selection: bool,
) -> CanvasEsc {
    if has_rel_link {
        CanvasEsc::RelLink
    } else if has_relationship {
        CanvasEsc::Relationship
    } else if focus_depth > 0 {
        CanvasEsc::Focus
    } else if has_selection {
        CanvasEsc::Selection
    } else {
        CanvasEsc::Nothing
    }
}

/// 概念图 `Tab`/`Enter` 要下发的命令：**一步**建出「概念 + 命题线」。
///
/// 绝不产生 `parent_id`——概念图的层级只由关系线表达，多一个 `parent_id` 就等于在命题
/// 网里埋了一棵看不见的树（大纲、搜索、导出都会跟着歪）。`Tab` 落下方、`Enter` 落右侧：
/// 命题网没有「同级」，两个键的差别只在落点方向。`anchor` 是发起概念的屏幕无关盒
/// `(x, y, w, h)`。
pub(crate) fn concept_relative_command(
    from: &str,
    kind: AddKind,
    anchor: (f32, f32, f32, f32),
) -> Command {
    let (x, y, w, _h) = anchor;
    let (x, y) = match kind {
        AddKind::Child => (x, y + 56.0),
        AddKind::Sibling => (x + w + 56.0, y),
    };
    Command::AddLinkedConcept {
        from: from.to_string(),
        text: CONCEPT_PLACEHOLDER.into(),
        x,
        y,
    }
}

impl Canvas {
    /// 把节点选中收成单选，并且**先清掉另外两条道**（M26 的互斥不变量）。
    ///
    /// 画布上有十来个入口会「把某个节点变成当前选中的那个」：悬停工具条、折叠圆点、
    /// 连线拖拽、新建主题、大纲的缩进/重排……它们以前各自裸写 `self.sel = ...`，
    /// 关系线/注解还选着时就会留下两道非空，下一次 `Delete` 按优先级（关系线 → 注解
    /// → 节点）删错对象。**所有「选一个节点」的入口都必须走这里**。
    pub(crate) fn select_node_only(&mut self, idx: u32, reason: &str) {
        self.clear_foreign_selection(SelectionLane::Node, reason);
        self.sel.replace(vec![idx]);
    }

    /// 清掉除 `keep` 外的另外两条选择道（打磨轮 49 / M26）。
    ///
    /// 只在实际清掉时写轨迹：同一帧里几次调用不会把证据文件刷满。
    pub(crate) fn clear_foreign_selection(&mut self, keep: SelectionLane, reason: &str) -> bool {
        let [rel, ann, nodes] = foreign_lanes(
            keep,
            !self.relationship.is_empty(),
            self.annotations.len(),
            self.sel.ids().len(),
        );
        if rel {
            self.relationship.clear();
        }
        if ann {
            self.annotations.clear();
        }
        let mut changed = rel || ann;
        if nodes {
            changed |= self.sel.clear();
        }
        if rel {
            crate::trace::event(&format!(
                "selection_clear lane=relationship reason={reason}"
            ));
        }
        if ann {
            crate::trace::event(&format!("selection_clear lane=annotation reason={reason}"));
        }
        if nodes {
            crate::trace::event(&format!("selection_clear lane=node reason={reason}"));
        }
        changed
    }

    /// 非编辑态落字即进编辑（`docs/28` §5.2 的 typing-to-edit）。
    ///
    /// 返回本帧是否吃掉了这条输入。只认主选：没选中任何东西时，落字没有落点。
    pub(crate) fn type_to_edit(&mut self, ui: &egui::Ui, viewport: [f32; 2]) -> bool {
        if self.editor.is_some() || self.input_blocked || self.confirm_delete.is_some() {
            return false;
        }
        let Some(idx) = self.sel.primary() else {
            return false;
        };
        let mods = ui.input(|i| i.modifiers);
        let Some(start) = ui.input_mut(|i| take_type_start(&mut i.events, mods)) else {
            return false;
        };
        self.start_edit(idx, viewport);
        let Some(ed) = self.editor.as_mut() else {
            return false;
        };
        match start {
            // 等价于「全选 + 输入」：整段文字换成这一条，提交时仍是一条 `SetText`。
            TypeStart::Replace(text) => ed.text = text,
            TypeStart::Compose => {
                // 组合串还没有着落：草稿清空，原文也一起留空——用户若什么都没组合出来
                // 就提交，等于「没改过」，不会把节点原来的文字抹掉。
                ed.text.clear();
                ed.original.clear();
            }
        }
        ed.laid_out.clear();
        // 立刻按新草稿重排：编辑器这一帧就要按新的字数把盒子撑开。
        self.sync_editing_layout();
        true
    }

    /// `Shift+Enter` 的软换行：在 `TextEdit` 的光标处插 `\n`，光标停在换行之后。
    ///
    /// 必须由画布自己插：编辑态的 `Enter` 归「提交 + 新建同级」，而且按键在到达
    /// `TextEdit` 之前就被我们摘走了，换行没有别的来源。
    pub(crate) fn insert_soft_newline(&mut self, ui: &egui::Ui) -> bool {
        let Some(ed) = self.editor.as_mut() else {
            return false;
        };
        let edit_id = node_edit_id(&ed.id);
        let mut state =
            egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit_id).unwrap_or_default();
        let sel = state
            .cursor
            .char_range()
            .map(|r| (r.primary.index.0, r.secondary.index.0));
        let (text, caret) = soft_newline_insert(&ed.text, sel);
        ed.text = text;
        ed.laid_out.clear();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(caret),
            )));
        state.store(ui.ctx(), edit_id);
        // 多了一行 ⇒ 节点盒高度要跟着长，否则文字会溢到盒子外面。
        self.sync_editing_layout();
        true
    }

    /// 待确认删除的弹窗。返回是否改动了文档。
    ///
    /// `Esc` 由这里自己接：画布自有的弹窗不在壳层 `overlay::State` 里，走不到
    /// `dialog::route_escape`（`main.rs` 调的 `any_modal` 不含画布），不接就等于
    /// 「只能点取消」。
    pub(crate) fn confirm_delete_ui(&mut self, ui: &egui::Ui) -> bool {
        let Some(pending) = self.confirm_delete.clone() else {
            return false;
        };
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            crate::trace::event(&format!(
                "delete-confirm cancel count={} reason=esc",
                pending.count
            ));
            self.confirm_delete = None;
            self.request_canvas_focus();
            return true;
        }
        let ch = crate::chrome::chrome_of(&self.session.theme);
        let faint = ch.faint;
        let actions = [
            crate::dialog::DialogAction::new("取消", crate::ui_kit::ButtonTone::Secondary),
            crate::dialog::DialogAction::new("删除", crate::ui_kit::ButtonTone::Danger),
        ];
        // 默认焦点落在「取消」：这个框挡的是不可逆动作，回车不该是顺手就中的那条路。
        let title = format!("删除 {} 个节点？", pending.count);
        let spec = crate::dialog::DialogSpec::new(&title).default_action(0);
        let out = crate::dialog::show(ui.ctx(), ch, "canvas-delete", spec, &actions, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "其中包含子节点，会一起删掉（共 {} 个）。",
                    pending.count
                ))
                .color(faint),
            );
            ui.add_space(6.0);
            ui.label(egui::RichText::new("删完可以 Ctrl+Z 撤销。").color(faint));
        });
        match out.action {
            Some(1) => {
                crate::trace::event(&format!("delete-confirm accept count={}", pending.count));
                self.confirm_delete = None;
                let ids = pending.ids;
                let ok = self.delete_now(&ids);
                self.request_canvas_focus();
                ok
            }
            Some(_) => {
                crate::trace::event(&format!("delete-confirm cancel count={}", pending.count));
                self.confirm_delete = None;
                self.request_canvas_focus();
                true
            }
            None if out.closed => {
                crate::trace::event(&format!("delete-confirm cancel count={}", pending.count));
                self.confirm_delete = None;
                self.request_canvas_focus();
                true
            }
            None => false,
        }
    }

    /// 思维导图的全选（`Ctrl+A`）：当前**可见**的全部节点。
    ///
    /// `Scene` 里只有看得见的节点（折叠起来的子树根本不在场景里），所以「不跨折叠」
    /// 天然成立，不需要再查一遍树。主选尽量保留，关系线/注解的选中一并清掉。
    pub(crate) fn select_all_visible(&mut self) -> bool {
        let all: Vec<u32> = (0..self.scene.ids.len() as u32).collect();
        if all.is_empty() {
            return false;
        }
        let primary = self
            .sel
            .primary()
            .filter(|&p| all.contains(&p))
            .or_else(|| self.scene.find(&self.session.doc.root_id));
        if !self.sel.replace(select_all_indices(all.len(), primary)) {
            return false;
        }
        self.annotations.clear();
        self.relationship.clear();
        self.refocus_canvas = true;
        true
    }

    /// 概念图里 `Tab`/`Enter`：加一颗概念 + 一条命题线，并立刻进链接短语编辑态。
    ///
    /// 短语留空确认会把这条线一起撤掉、概念留着（`docs/30` §5.9），与拖到空白处建线
    /// 的手感一致。全程不碰 `parent_id`。
    pub(crate) fn add_concept_relative(&mut self, kind: AddKind) -> bool {
        if !self.concept_map() {
            return false;
        }
        // 从主选概念出发；主选不是概念（带 `parent_id`）或没有主选时退回根概念。
        let from = self
            .primary_id()
            .filter(|id| {
                self.session
                    .doc
                    .node(id)
                    .is_some_and(|n| n.parent_id.is_none())
            })
            .unwrap_or_else(|| self.session.doc.root_id.clone());
        let Some(idx) = self.scene.find(&from) else {
            return false;
        };
        let r = self.scene.rect(idx);
        let cmd = concept_relative_command(&from, kind, (r.x, r.y, r.w, r.h));
        let before = self.session.doc.doc_order();
        if !self.apply_structural(Some(cmd)) {
            return false;
        }
        let Some(new_id) = self
            .session
            .doc
            .doc_order()
            .into_iter()
            .find(|id| !before.contains(id))
        else {
            return false;
        };
        crate::trace::event(&format!(
            "canvas_concept_relative from={from} new={new_id} kind={kind:?}"
        ));
        self.select_node(&new_id);
        // 边已由 `AddLinkedConcept` 建好，收尾只负责重建场景 + 立起短语编辑框。
        self.finish_new_link(from, new_id)
    }
}
