//! 测试夹具：用短 id 直接搭一棵树，省去走命令层的样板代码。
//!
//! id 是测试断言的坐标（`scene.index["a1"]`），所以由调用方给定；
//! 建出来的文档仍过 `Document::from_parts` 的结构校验，不会造出非法父子关系。

use std::collections::HashMap;

use crate::model::{
    ArrowHead, Boundary, DocMeta, Node, Position, Relationship, StyleOverride, Summary,
};
use crate::Document;

pub(crate) struct Builder {
    nodes: HashMap<String, Node>,
    root_id: String,
    meta: DocMeta,
}

impl Builder {
    /// 只有主根的文档：根 id 为 `"r"`。
    pub fn root(root_text: &str) -> Self {
        let mut r = Node::new("r".into(), None, root_text);
        r.modified_at = 1;
        let mut nodes = HashMap::new();
        nodes.insert("r".to_string(), r);
        Self {
            nodes,
            root_id: "r".to_string(),
            meta: DocMeta::default(),
        }
    }

    pub fn child(mut self, parent: &str, id: &str, text: &str) -> Self {
        self.kid(parent, id, text);
        self
    }

    pub fn styled(mut self, parent: &str, id: &str, text: &str, style: StyleOverride) -> Self {
        self.kid(parent, id, text);
        self.nodes.get_mut(id).unwrap().style = Some(style);
        self
    }

    pub fn collapsed(mut self, parent: &str, id: &str, text: &str) -> Self {
        self.kid(parent, id, text);
        self.nodes.get_mut(id).unwrap().collapsed = true;
        self
    }

    /// 自由主题（浮动分支根，无父节点）；`pos = None` 时由布局自动摆放。
    pub fn float(mut self, id: &str, text: &str, pos: Option<(f32, f32)>) -> Self {
        let mut n = Node::new(id.to_string(), None, text);
        n.pos = pos.map(|(x, y)| Position { x, y });
        n.modified_at = 1;
        self.nodes.insert(id.to_string(), n);
        self
    }

    /// 给自由主题挂子节点（`parent` 必须是 [`float`](Builder::float) 建出的 id）。
    pub fn under(mut self, parent: &str, id: &str, text: &str) -> Self {
        self.kid(parent, id, text);
        self
    }

    fn kid(&mut self, parent: &str, id: &str, text: &str) {
        let mut n = Node::new(id.to_string(), Some(parent.to_string()), text);
        n.modified_at = 1;
        self.nodes
            .get_mut(parent)
            .unwrap_or_else(|| panic!("父节点不存在：{parent}"))
            .children
            .push(id.to_string());
        self.nodes.insert(id.to_string(), n);
    }

    pub fn relationship(mut self, from: &str, rel_id: &str, to: &str, label: Option<&str>) -> Self {
        self.nodes
            .get_mut(from)
            .unwrap()
            .relationships
            .push(Relationship {
                id: rel_id.to_string(),
                to: to.to_string(),
                label: label.map(str::to_string),
                color: None,
                head: ArrowHead::Forward,
                ..Default::default()
            });
        self
    }

    pub fn boundary(mut self, parent: &str, id: &str, targets: &[&str]) -> Self {
        self.nodes
            .get_mut(parent)
            .unwrap()
            .boundaries
            .push(Boundary {
                id: id.to_string(),
                targets: targets.iter().map(|s| s.to_string()).collect(),
                color: None,
            });
        self
    }

    pub fn summary(mut self, parent: &str, id: &str, targets: &[&str], text: &str) -> Self {
        self.nodes.get_mut(parent).unwrap().summaries.push(Summary {
            id: id.to_string(),
            targets: targets.iter().map(|s| s.to_string()).collect(),
            text: text.to_string(),
        });
        self
    }

    pub fn labels(mut self, id: &str, labels: &[&str]) -> Self {
        self.nodes.get_mut(id).unwrap().labels = labels.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn note(mut self, id: &str, note: &str) -> Self {
        self.nodes.get_mut(id).unwrap().note = Some(note.to_string());
        self
    }

    pub fn link(mut self, id: &str, link: &str) -> Self {
        self.nodes.get_mut(id).unwrap().link = Some(link.to_string());
        self
    }

    pub fn icon(mut self, id: &str, icon: &str) -> Self {
        self.nodes.get_mut(id).unwrap().icon = Some(icon.to_string());
        self
    }

    pub fn build(self) -> Document {
        Document::from_parts(None, self.meta, self.root_id.clone(), self.nodes)
            .expect("测试夹具必须构造出合法文档")
    }
}

/// 根 + 三分支（`a` 带两叶、`b` 带一叶）的常用形状。
pub(crate) fn simple() -> Document {
    Builder::root("根")
        .child("r", "a", "分支甲")
        .child("r", "b", "分支乙")
        .child("r", "c", "分支丙")
        .child("a", "a1", "叶子一")
        .child("a", "a2", "叶子二")
        .child("b", "b1", "叶子三")
        .build()
}
