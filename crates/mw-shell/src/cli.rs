//! 无界面 CLI：把 `.zmind` 批量导出为文本格式或矢量 PDF，便于脚本/CI 使用。
//!
//! ```text
//! mw-shell --export-text a.zmind b.zmind --format opml --out-dir out
//! mw-shell --export-text a.zmind --format md --out a.md
//! mw-shell --export-pdf a.zmind --out a.pdf
//! ```
//! PDF 是字形轮廓 + 路径，不需要 GPU；位图 PNG 仍需窗口内的离屏合成器。

use std::path::{Path, PathBuf};

use mw_core::TextMeasurer;

use crate::export_presets;

pub fn run(args: impl Iterator<Item = String>) -> Option<i32> {
    let args: Vec<String> = args.collect();
    if args.iter().any(|a| a == "--export-pdf") {
        return Some(match export_pdf(&args) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("[cli] {e}");
                2
            }
        });
    }
    if !args.iter().any(|a| a == "--export-text") {
        return None;
    }
    match export(&args) {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("[cli] {e}");
            Some(2)
        }
    }
}

fn usage() -> String {
    "用法：mw-shell --export-text <input.zmind...> --format <md|txt|opml|mm> [--out file | --out-dir dir]\n  或：mw-shell --export-pdf <input.zmind...> [--out file.pdf | --out-dir dir]".into()
}

fn export(args: &[String]) -> Result<(), String> {
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut format = String::new();
    let mut out: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--export-text" | "-e" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    inputs.push(PathBuf::from(&args[i]));
                    i += 1;
                }
                continue;
            }
            "--format" | "-f" => {
                i += 1;
                format = args.get(i).ok_or_else(|| usage().to_string())?.clone();
            }
            "--out" | "-o" => {
                i += 1;
                out = Some(PathBuf::from(
                    args.get(i).ok_or_else(|| usage().to_string())?,
                ));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(
                    args.get(i).ok_or_else(|| usage().to_string())?,
                ));
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => return Err(format!("未知参数：{other}\n{}", usage())),
        }
        i += 1;
    }
    if inputs.is_empty() {
        return Err(format!("没有输入文件\n{}", usage()));
    }
    if format.is_empty() {
        format = out
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("md")
            .to_string();
    }
    let format = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => "md",
        "txt" | "outline" => "txt",
        "opml" => "opml",
        "mm" | "freemind" => "mm",
        other => return Err(format!("不支持的文本格式：{other}")),
    };
    if inputs.len() > 1 && out.is_some() {
        return Err("多个输入时请用 --out-dir，而不是 --out".into());
    }
    for input in inputs {
        let path = match out.clone() {
            Some(p) => p,
            None => {
                let dir = out_dir.clone().unwrap_or_else(|| PathBuf::from("."));
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("导出");
                export_presets::unique_path(&dir, stem, format)
            }
        };
        export_one(&input, format, &path)?;
        println!("{} -> {}", input.display(), path.display());
    }
    Ok(())
}

pub(crate) fn export_one(input: &Path, format: &str, out: &Path) -> Result<(), String> {
    let loaded = mw_format::load(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let text = match format {
        "md" => mw_io::export_markdown(&loaded.doc),
        "txt" => mw_io::export_outline(&loaded.doc),
        "opml" => mw_io::export_opml(&loaded.doc),
        "mm" => mw_io::export_freemind(&loaded.doc),
        other => return Err(format!("不支持的文本格式：{other}")),
    };
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(out, text).map_err(|e| format!("{}: {e}", out.display()))
}

/// PDF 批导出：每个输入一份 A4 多页矢量 PDF。
fn export_pdf(args: &[String]) -> Result<(), String> {
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--export-pdf" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    inputs.push(PathBuf::from(&args[i]));
                    i += 1;
                }
                continue;
            }
            "--out" | "-o" => {
                i += 1;
                out = Some(PathBuf::from(
                    args.get(i).ok_or_else(|| usage().to_string())?,
                ));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(
                    args.get(i).ok_or_else(|| usage().to_string())?,
                ));
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => return Err(format!("未知参数：{other}\n{}", usage())),
        }
        i += 1;
    }
    if inputs.is_empty() {
        return Err(format!("没有输入文件\n{}", usage()));
    }
    if inputs.len() > 1 && out.is_some() {
        return Err("多个输入时请用 --out-dir，而不是 --out".into());
    }
    for input in inputs {
        let path = match out.clone() {
            Some(p) => p,
            None => {
                let dir = out_dir.clone().unwrap_or_else(|| PathBuf::from("."));
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("导出");
                export_presets::unique_path(&dir, stem, "pdf")
            }
        };
        let (pages, bytes) = export_pdf_one(&input, &path)?;
        println!(
            "{} -> {}（{pages} 页，{bytes} 字节）",
            input.display(),
            path.display()
        );
    }
    Ok(())
}

pub(crate) fn export_pdf_one(input: &Path, out: &Path) -> Result<(usize, usize), String> {
    let loaded = mw_format::load(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let theme = mw_core::theme_for(&loaded.doc.meta);
    // 用真实 DirectWrite 度量排版：MonoMeasurer 会低估中文宽度，导出后字会顶到卡片边缘。
    let real = mw_render::DWrite::new().map(mw_render::DWriteMeasurer::new);
    let measurer: &dyn TextMeasurer = match &real {
        Some(m) => m,
        None => &mw_core::MonoMeasurer,
    };
    let mut scene = mw_core::build_scene(
        &loaded.doc,
        &loaded.doc.root_id,
        &loaded.doc.meta.layout,
        &theme,
        measurer,
        None,
    );
    // 命令行导出也是导出：聚焦问题同样要带上（docs/32 §6）。
    crate::export::attach_focus_note(&mut scene, &loaded.doc, &theme);
    let enc = crate::export_pdf::encode_scene(&scene, &theme, None)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(out, &enc.bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((enc.pages, enc.bytes.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_exports_opml_from_a_real_zmind() {
        let dir = std::env::temp_dir().join(format!("mw-cli-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = mw_core::Document::new("CLI 根");
        let zmind = dir.join("a.zmind");
        mw_format::save(&doc, &zmind).unwrap();
        let out = dir.join("a.opml");
        export_one(&zmind, "opml", &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("<opml"));
        assert!(text.contains("CLI 根"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cli_exports_a_vector_pdf_from_a_real_zmind() {
        let dir = std::env::temp_dir().join(format!("mw-cli-pdf-{}", mw_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = mw_core::Document::new("PDF 根");
        let root = doc.root_id.clone();
        doc.nodes.get_mut(&root).unwrap().text = "PDF 根".into();
        for t in ["第一分支", "第二分支"] {
            let id = mw_core::new_id();
            let n = mw_core::Node::new(id.clone(), Some(root.clone()), t.to_string());
            doc.nodes.insert(id.clone(), n);
            doc.nodes.get_mut(&root).unwrap().children.push(id);
        }
        let zmind = dir.join("b.zmind");
        mw_format::save(&doc, &zmind).unwrap();
        let out = dir.join("b.pdf");
        let (pages, bytes) = export_pdf_one(&zmind, &out).unwrap();
        assert_eq!(pages, 1);
        assert!(bytes > 1000);
        let pdf = std::fs::read(&out).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Type /Page "));
        assert!(!text.contains("/Subtype /Image"));
        assert!(text.matches(" c\n").count() > 100, "文字必须变成曲线");
        let _ = std::fs::remove_dir_all(dir);
    }
}
