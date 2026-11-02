//! 图标标记定义（文档 02 §7：优先级 / 任务状态 / 星标）。
//!
//! `key` 持久化在 `node.icon`，`glyph` 是排版用字形——全部为印刷符号（①○◐✓★?!），
//! 不含 emoji：emoji 在 Windows 上会落到 Segoe UI Emoji 的彩色位图，缩放即发虚（docs/12 D6）。

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconDef {
    pub key: &'static str,
    pub glyph: &'static str,
    pub color: &'static str,
    pub label: &'static str,
}

pub const ICONS: [IconDef; 12] = [
    IconDef {
        key: "priority-1",
        glyph: "①",
        color: "#e5484d",
        label: "优先级 1",
    },
    IconDef {
        key: "priority-2",
        glyph: "②",
        color: "#f2994a",
        label: "优先级 2",
    },
    IconDef {
        key: "priority-3",
        glyph: "③",
        color: "#f2c94c",
        label: "优先级 3",
    },
    IconDef {
        key: "priority-4",
        glyph: "④",
        color: "#2d9cdb",
        label: "优先级 4",
    },
    IconDef {
        key: "priority-5",
        glyph: "⑤",
        color: "#9b51e0",
        label: "优先级 5",
    },
    IconDef {
        key: "priority-6",
        glyph: "⑥",
        color: "#828282",
        label: "优先级 6",
    },
    IconDef {
        key: "task-todo",
        glyph: "○",
        color: "#8a93a6",
        label: "待办",
    },
    IconDef {
        key: "task-doing",
        glyph: "◐",
        color: "#f2994a",
        label: "进行中",
    },
    IconDef {
        key: "task-done",
        glyph: "✓",
        color: "#27ae60",
        label: "已完成",
    },
    IconDef {
        key: "star",
        glyph: "★",
        color: "#f2c94c",
        label: "星标",
    },
    IconDef {
        key: "question",
        glyph: "?",
        color: "#2d9cdb",
        label: "疑问",
    },
    IconDef {
        key: "exclaim",
        glyph: "!",
        color: "#eb5757",
        label: "重要",
    },
];

/// 按 key 查定义；未知名按「无图标」处理（导入的老文件可能带自定义 key）。
pub fn icon_def(key: Option<&str>) -> Option<&'static IconDef> {
    let key = key?;
    ICONS.iter().find(|i| i.key == key)
}

/// 图标占位宽度：字号 + 6（TS scene.ts 同一常量关系）。
pub fn icon_width(font_size: f32) -> f32 {
    font_size + 6.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_key_and_reject_unknown() {
        assert_eq!(icon_def(Some("star")).map(|i| i.glyph), Some("★"));
        assert_eq!(icon_def(Some("fire")), None);
        assert_eq!(icon_def(None), None);
    }

    #[test]
    fn every_color_is_parsable() {
        for i in ICONS.iter() {
            assert!(crate::theme::Rgba::parse(i.color).is_some(), "{}", i.key);
        }
    }
}
