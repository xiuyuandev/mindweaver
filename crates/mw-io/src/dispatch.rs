//! 按文件扩展名分发保存 / 打开（`docs/34` §6）。
//!
//! | 扩展名 | 格式 | 外部可开 |
//! | --- | --- | --- |
//! | `.xmind` | XMind Zen 包 + 保真槽 | XMind |
//! | `.cxl` | CmapTools CXL + 保真槽 | CmapTools |
//! | `.md` | Markdown（大纲/笔记） | Obsidian / 编辑器 |
//! | `.zmind` | 旧版原生容器（兼容保留） | — |
//!
//! 现有 `.zmind` 测试路径不受影响（仍走 `mw_format`）；新保存默认写开放格式
//! （思维导图 `.xmind`、概念图 `.cxl`），见 [`default_ext`]。

use std::io::Write;
use std::path::Path;

use mw_core::Document;

/// 文档的落盘格式（由扩展名决定）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocFormat {
    /// XMind Zen（思维导图）。
    Xmind,
    /// CmapTools CXL（概念图）。
    Cxl,
    /// Markdown（大纲导出 / 笔记）。
    Markdown,
    /// 旧版 `.zmind` 原生容器（兼容）。
    Zmind,
}

/// 按扩展名判定格式。未知扩展名回落 `.zmind`（旧版原生）。
pub fn format_of(path: &Path) -> DocFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xmind" => DocFormat::Xmind,
        "cxl" | "cmap" => DocFormat::Cxl,
        "md" | "markdown" => DocFormat::Markdown,
        _ => DocFormat::Zmind,
    }
}

/// 新文档默认扩展名：概念图 → `.cxl`，思维导图 → `.xmind`。
pub fn default_ext(doc: &Document) -> &'static str {
    if doc.meta.layout.kind == "conceptMap" {
        ".cxl"
    } else {
        ".xmind"
    }
}

/// 保存文档到路径（按扩展名选格式）。返回写入的字节数。
pub fn save_document(doc: &Document, path: &Path) -> Result<usize, String> {
    match format_of(path) {
        DocFormat::Xmind => {
            let bytes = crate::xmind::save_xmind(doc).map_err(|e| e.to_string())?;
            write_safe(path, &bytes)?;
            Ok(bytes.len())
        }
        DocFormat::Cxl => {
            let xml = crate::cmap::save_cxl(doc).map_err(|e| e.to_string())?;
            write_safe(path, xml.as_bytes())?;
            Ok(xml.len())
        }
        DocFormat::Markdown => {
            let md = crate::export::export_markdown(doc);
            write_safe(path, md.as_bytes())?;
            Ok(md.len())
        }
        DocFormat::Zmind => {
            // 旧版原生容器：原子写 + .bak 由 `mw_format::save` 内建。
            mw_format::save(doc, path).map_err(|e| e.to_string())?;
            Ok(0)
        }
    }
}

/// 从路径加载文档（按扩展名选格式）。
pub fn load_document(path: &Path) -> Result<Document, String> {
    match format_of(path) {
        DocFormat::Xmind => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            crate::xmind::load_xmind(&bytes).map_err(|e| e.to_string())
        }
        DocFormat::Cxl => {
            let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            crate::cmap::load_cxl(&xml).map_err(|e| e.to_string())
        }
        DocFormat::Markdown => {
            let md = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            crate::import::import_markdown(&md).map_err(|e| e.to_string())
        }
        DocFormat::Zmind => {
            let loaded = mw_format::load(path).map_err(|e| e.to_string())?;
            Ok(loaded.doc)
        }
    }
}

/// 崩溃安全写：临时文件 → 更名。任一时点磁盘上至少有一份完整文件。
fn write_safe(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::{model::Relationship, DocMeta, LayoutSettings, Node};
    use std::collections::HashMap;

    /// 测试落地目录：**仓库内 target/tmp 优先**（同 `mw-format::tests::scratch`）——
    /// 本机用户 Temp 对测试进程偶发拒写（os error 5），会让写盘用例假失败。
    fn scratch(prefix: &str) -> std::path::PathBuf {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/tests");
        let base = if std::fs::create_dir_all(&base).is_ok() {
            base
        } else {
            std::env::temp_dir()
        };
        let dir = base.join(format!("{}-{}", prefix, mw_core::new_id()));
        std::fs::create_dir_all(&dir).expect("测试临时目录必须能创建");
        dir
    }

    fn mind_doc() -> Document {
        let mut nodes = HashMap::new();
        let mut root = Node::new("r".into(), None, "根");
        let mut c = Node::new("c1".into(), Some("r".into()), "子");
        c.note = Some("备注".into());
        c.labels = vec!["重点".into()];
        root.children = vec!["c1".into()];
        nodes.insert("r".into(), root);
        nodes.insert("c1".into(), c);
        let meta = DocMeta {
            title: "导图".into(),
            theme: "snow".into(),
            theme_accent: None,
            theme_overrides: None,
            layout: LayoutSettings::default(),
        };
        Document::from_parts_full(None, meta, "r".into(), nodes, Vec::new(), Vec::new()).unwrap()
    }

    fn concept_doc() -> Document {
        let mut nodes = HashMap::new();
        nodes.insert("a".into(), Node::new("a".into(), None, "A"));
        let mut b = Node::new("b".into(), None, "B");
        b.relationships.push(Relationship {
            id: "p".into(),
            to: "a".into(),
            label: Some("是".into()),
            ..Default::default()
        });
        nodes.insert("b".into(), b);
        let meta = DocMeta {
            title: "概念图".into(),
            theme: "snow".into(),
            theme_accent: None,
            theme_overrides: None,
            layout: LayoutSettings {
                kind: "conceptMap".into(),
                focus_question: Some("谁是谁？".into()),
                ..Default::default()
            },
        };
        Document::from_parts_full(None, meta, "a".into(), nodes, Vec::new(), Vec::new()).unwrap()
    }

    #[test]
    fn format_of_dispatches_by_extension() {
        assert_eq!(format_of(Path::new("a.xmind")), DocFormat::Xmind);
        assert_eq!(format_of(Path::new("a.cxl")), DocFormat::Cxl);
        assert_eq!(format_of(Path::new("a.md")), DocFormat::Markdown);
        assert_eq!(format_of(Path::new("a.zmind")), DocFormat::Zmind);
        assert_eq!(format_of(Path::new("a.bin")), DocFormat::Zmind);
    }

    #[test]
    fn default_ext_by_doc_kind() {
        assert_eq!(default_ext(&mind_doc()), ".xmind");
        assert_eq!(default_ext(&concept_doc()), ".cxl");
    }

    #[test]
    fn save_load_roundtrip_xmind_and_cxl() {
        let dir = scratch("mw-dispatch");

        // 思维导图 → .xmind 往返。
        let mp = dir.join("mind.xmind");
        save_document(&mind_doc(), &mp).unwrap();
        let back = load_document(&mp).unwrap();
        assert_eq!(back.meta.title, "导图");
        assert_eq!(back.nodes.len(), 2);
        assert_eq!(back.nodes.get("c1").unwrap().note.as_deref(), Some("备注"));

        // 概念图 → .cxl 往返。
        let cp = dir.join("concept.cxl");
        save_document(&concept_doc(), &cp).unwrap();
        let back = load_document(&cp).unwrap();
        assert_eq!(back.meta.layout.focus_question.as_deref(), Some("谁是谁？"));
        assert_eq!(back.nodes.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
