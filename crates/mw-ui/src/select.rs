//! 多选集合：一个有序 id 列表，**末尾就是主选**。
//!
//! 用有序 `Vec` 而不是 `HashSet` 有两个理由：① 主选（决定 Tab/Enter/F2 作用在哪个
//! 节点上、也决定框选后编辑谁）必须有确定答案；② 拖拽多根时插入顺序要稳定，
//! 集合遍历顺序不稳定的话，同一下手势两次会落出两种树形。

/// 场景索引下的选中集合（`u32` 是 `Scene` 的稠密索引，不是文档 id）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    ids: Vec<u32>,
}

impl Selection {
    pub fn one(idx: u32) -> Self {
        Self { ids: vec![idx] }
    }

    pub fn ids(&self) -> &[u32] {
        &self.ids
    }

    pub fn primary(&self) -> Option<u32> {
        self.ids.last().copied()
    }

    pub fn contains(&self, idx: u32) -> bool {
        self.ids.contains(&idx)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn clear(&mut self) -> bool {
        let had = !self.ids.is_empty();
        self.ids.clear();
        had
    }

    /// 整组替换（框选结果）。顺序即传入顺序，末尾为主选。
    pub fn replace(&mut self, ids: Vec<u32>) -> bool {
        if self.ids == ids {
            return false;
        }
        self.ids = ids;
        true
    }

    /// 单击一个节点。
    ///
    /// `additive`（Ctrl / Shift）→ 切换成员；否则**若已在组内则只提为主选**，
    /// 不拆散多选组——旧版这里踩过坑：多选后点其中一个是想继续操作这一组，
    /// 不是想丢掉其它选择。
    pub fn click(&mut self, idx: u32, additive: bool) -> bool {
        if additive {
            return self.toggle(idx);
        }
        if self.contains(idx) {
            return self.make_primary(idx);
        }
        self.replace(vec![idx])
    }

    /// 带 Alt 的单击：Alt 只减选，不切换。点未选中的节点是空操作。
    pub fn click_with(&mut self, idx: u32, additive: bool, subtract: bool) -> bool {
        if subtract {
            return self.remove(idx);
        }
        self.click(idx, additive)
    }

    /// 从组里摘掉一个（删除节点后调用）。
    pub fn remove(&mut self, idx: u32) -> bool {
        let before = self.ids.len();
        self.ids.retain(|&i| i != idx);
        self.ids.len() != before
    }

    fn toggle(&mut self, idx: u32) -> bool {
        if self.contains(idx) {
            self.remove(idx)
        } else {
            self.ids.push(idx);
            true
        }
    }

    fn make_primary(&mut self, idx: u32) -> bool {
        let last = self.ids.last() == Some(&idx);
        if last {
            return false;
        }
        self.ids.retain(|&i| i != idx);
        self.ids.push(idx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;

    #[test]
    fn alt_click_only_subtracts() {
        let mut sel = Selection::default();
        sel.replace(vec![3, 7, 9]);
        assert!(sel.click_with(7, false, true));
        assert_eq!(sel.ids(), [3, 9]);
        assert!(!sel.click_with(7, true, true), "不在组里，减选是空操作");
        assert_eq!(sel.ids(), [3, 9]);
    }

    #[test]
    fn plain_click_on_an_unselected_node_replaces_the_group() {
        let mut sel = Selection::one(1);
        assert!(sel.click(5, false));
        assert_eq!(sel.ids(), [5]);
    }

    #[test]
    fn plain_click_inside_the_group_keeps_it_and_only_repoints_primary() {
        let mut sel = Selection::default();
        sel.replace(vec![3, 7, 9]);
        assert!(sel.click(3, false));
        assert_eq!(sel.ids(), [7, 9, 3]);
        assert_eq!(sel.primary(), Some(3));
    }

    #[test]
    fn additive_click_toggles_membership() {
        let mut sel = Selection::one(2);
        assert!(sel.click(4, true));
        assert_eq!(sel.ids(), [2, 4]);
        assert!(sel.click(2, true), "再点一次应移出组");
        assert_eq!(sel.ids(), [4]);
        assert!(sel.click(2, true), "第三次点回来，排在末尾成为主选");
        assert_eq!(sel.ids(), [4, 2]);
        assert_eq!(sel.primary(), Some(2));
    }

    #[test]
    fn no_op_clicks_report_unchanged() {
        let mut sel = Selection::one(6);
        assert!(!sel.click(6, false), "已经主选的节点再点一次不该算变化");
        assert!(!sel.replace(vec![6]));
    }

    #[test]
    fn removal_leaves_the_previous_member_primary() {
        let mut sel = Selection::default();
        sel.replace(vec![1, 2]);
        assert!(sel.remove(2));
        assert_eq!(sel.primary(), Some(1));
    }
}
