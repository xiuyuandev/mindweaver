//! `.zmind` 容器读写（格式规范见文档 05 §4）。
//!
//! 容器为 ZIP：`mimetype`（STORED，首条目）+ `manifest.json` + `content.json`。
//! 保存采用「临时文件 → fsync → 旧文件转 .bak → 更名 → 目录 fsync」的崩溃安全流程：
//! 任一时点磁盘上至少存在一份完整文件（05 §6）。
//!
//! 读侧刻意比写侧宽松：一份带「本版本不认识的键 / 枚举取值」的文件必须能打开
//! （那是「新版本写、旧版本读」的日常），而且那些不认识的键要在再保存时原样写回去。

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mw_core::{DocMeta, Document, LayoutSettings, Node, NoteCard};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const FORMAT_VERSION: u32 = 1;
pub const APP_ID: &str = "MindWeaver/0.1.0";
pub const MIME: &str = "application/vnd.mindweaver.zmind";

/// 概念图的专属数据形状（`docs/33` §3）。
mod concept_map;
pub use concept_map::{ConceptEntry, ConceptMapBlock, PropositionEntry};

const MAX_TOTAL_UNCOMPRESSED: u64 = 256 * 1024 * 1024;
/// 条目数上限。卡片笔记每张一个 `notes/*.md`，1000 张是常规量级；
/// 总量另有 `MAX_TOTAL_UNCOMPRESSED` 守卫，条目数不必再卡在 1024。
const MAX_ENTRIES: usize = 20_000;
const MAX_ENTRY: u64 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("读写文件失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("压缩包损坏：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("文件内容解析失败：{0}")]
    Json(#[from] serde_json::Error),
    #[error("此文件由更新版本的织思创建（格式版本 {0}），请升级后再打开")]
    TooNew(u32),
    #[error("无效的 .zmind 文件：{0}")]
    Invalid(String),
    #[error("文档结构损坏：{0}")]
    Core(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub format_version: u32,
    pub app: String,
    pub doc_id: String,
    pub title: String,
    pub created_at: i64,
    pub modified_at: i64,
    pub node_count: u64,
    /// 卡片笔记数量；老文件没有这个键时按 0 读。
    #[serde(default)]
    pub note_count: u64,
    /// 资源包条目数（同哈希只算一份）。
    #[serde(default)]
    pub asset_count: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct FileDoc {
    schema: u32,
    root_id: String,
    nodes: HashMap<String, Node>,
    theme: String,
    /// 没有自定义强调色时**整个键都不写**：老文档的 `content.json` 保持原样。
    #[serde(skip_serializing_if = "Option::is_none")]
    theme_accent: Option<String>,
    /// 主题微调同理：没设置时整个键省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme_overrides: Option<mw_core::model::ThemeOverrides>,
    layout: LayoutSettings,
    /// 卡片笔记。正文同时写进 `notes/<id>.md`，这里保留一份是为了加载快与向后兼容。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cards: Vec<NoteCard>,
    /// 概念分组（CM-18，`docs/32` §13）。没有分组时整个键都不写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<mw_core::model::ConceptGroup>,
    /// 概念图的专属数据形状（`docs/33` §3）。
    ///
    /// **读**：这块存在时它是权威（`nodes` 被忽略）；缺块回落到旧形状，老文件
    /// 一个字节都不用改。**写**：概念图文档写出这块，同时把 `nodes` 一起写
    /// （兼容镜像，老版本的织思打开还能看见）。没写过就是 `None`，整个键不落盘
    /// ——思维导图的 `content.json` 逐字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    concept_map: Option<concept_map::ConceptMapBlock>,
    /// 本版本不认识的顶层键（更新版本的织思写进来的字段）。
    ///
    /// `flatten` 让它们读进来、存出去的形状都不变：**新版本写的字段被旧版本打开再存不会消失**。
    /// 缺这个键、或者一个都没有，就是老行为（序列化出来与旧版逐字节相同——摊在已知字段之后）。
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

pub struct LoadedDoc {
    pub doc: Document,
    pub manifest: Manifest,
}

/// 一份文档里「本版本不认识」的键。
///
/// 保真目标：新版本写的字段，被旧版本打开、编辑、再保存之后仍然留在文件里。
/// 这些键唯一的来源是「读过一次文件」，所以加载时捡起来、保存时写回去。
#[derive(Clone, Debug, Default)]
struct DocExtras {
    /// `content.json` 顶层（`FileDoc` 的 `flatten` 字段）。
    doc: serde_json::Map<String, serde_json::Value>,
    /// 每个节点自己的未知键：`Node` 在 `mw-core`，加不了 `flatten`，只能按 id 挂在这里。
    nodes: HashMap<String, serde_json::Map<String, serde_json::Value>>,
}

impl DocExtras {
    /// 一份干净的（老版本写的）文件不该有任何未知键——测试用它钉住「没有多捡东西」。
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.doc.is_empty() && self.nodes.is_empty()
    }
}

/// 进程内未知键账本的一条：这份文档**是从哪个文件读来的**，以及那次读到的未知键。
///
/// 为什么键是 `(docId, 源路径)` 而不只是 `docId`：用户把一份 `.zmind` 复制成两份、两个都打开，
/// 两份的 `docId` 相同。只按 id 记账，A 的未知键会被写到 B 上（把别人的字段搬进另一份文档，
/// 比丢字段严重）。所以保存时要求「账本记的源路径 == 这次的保存目标路径」才肯写回。
struct ExtrasEntry {
    doc_id: String,
    /// 源文件路径；`None` = 从字节读的（没有源文件，例如自动保存槽位里取出来的内容）。
    source: Option<PathBuf>,
    extras: DocExtras,
}

impl ExtrasEntry {
    /// 这次的保存目标是不是当初读进来的那个文件。
    fn same_file(&self, path: &Path) -> bool {
        let Some(source) = self.source.as_deref() else {
            // 不知道从哪读来的，就不敢往下写：丢未知键也比写错文件好。
            return false;
        };
        same_path(source, path)
    }
}

/// 两个路径是不是同一份文件。
///
/// 先 `canonicalize` 再比：同一份文件可能被写成 `C:\a\b.zmind` / `c:/a/./b.zmind` 两种样子。
/// 目标可能还不存在（首次保存）或规范化失败（权限/网络盘），那就退化成字面比较。
fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// 进程内未知键账本。
///
/// 为什么不挂在 `Document` 上：`Document` 在 `mw-core`，本包只许改 `mw-format`。
/// 为什么不在保存时再去读一遍旧文件：加载一定先于保存（打开 → 编辑 → 保存的必经路径），
/// 账本已经够用；每次保存多读一遍几 MB 的旧容器是白付的延迟。
const EXTRAS_CACHE_CAP: usize = 32;
static EXTRAS_CACHE: OnceLock<Mutex<Vec<ExtrasEntry>>> = OnceLock::new();

fn extras_cache() -> &'static Mutex<Vec<ExtrasEntry>> {
    EXTRAS_CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

/// 记下这份文档的未知键。**空的也要记**：否则同一个 id 上一次的残留会被下一次保存写出去。
fn remember_extras(doc_id: &str, source: Option<&Path>, extras: DocExtras) {
    let mut cache = extras_cache().lock().unwrap_or_else(|e| e.into_inner());
    // 同一个 (docId, 源路径) 只留最新一条；不同路径的同一份文档各记各的。
    if let Some(pos) = cache
        .iter()
        .position(|entry| entry.doc_id == doc_id && entry.source.as_deref() == source)
    {
        cache.remove(pos);
    }
    cache.push(ExtrasEntry {
        doc_id: doc_id.to_string(),
        source: source.map(Path::to_path_buf),
        extras,
    });
    if cache.len() > EXTRAS_CACHE_CAP {
        cache.remove(0);
    }
}

/// 取这份文档的账本条目（最近一次加载的那条）。仅测试用来断言「捡到了什么」。
#[cfg(test)]
fn cached_extras(doc_id: &str) -> Option<DocExtras> {
    let cache = extras_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache
        .iter()
        .rev()
        .find(|entry| entry.doc_id == doc_id)
        .map(|entry| entry.extras.clone())
}

/// 保存时真正肯写回去的未知键：**只有源路径与目标路径一致**才给。
///
/// 不匹配就返回空的——`另存为`、把副本存到别处、从字节读来再保存，这几种情况都会丢掉未知键，
/// 这是有意的降级：宁可少写一个不认识的字段，也不要把另一份文档的字段搬过来。
fn extras_for_save(doc_id: &str, path: &Path) -> DocExtras {
    let cache = extras_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache
        .iter()
        .rev()
        .find(|entry| entry.doc_id == doc_id && entry.same_file(path))
        .map(|entry| entry.extras.clone())
        .unwrap_or_default()
}

/// `Node` 序列化出来的键集合，也就是「本版本认识的节点字段」。
///
/// 拿 `Default` 去序列化而不是写死一张名单：`Node` 的字段没有 `skip_serializing_if`，
/// 每个字段都会出现，所以名单不会漏；`mw-core` 以后加字段这里自动跟上。
fn known_node_keys() -> &'static HashSet<String> {
    static KEYS: OnceLock<HashSet<String>> = OnceLock::new();
    KEYS.get_or_init(|| {
        serde_json::to_value(Node::default())
            .ok()
            .and_then(|v| v.as_object().map(|obj| obj.keys().cloned().collect()))
            .unwrap_or_default()
    })
}

/// 从原始的 `content.json`（已解成 Value）里挑出**节点级**未知键。
///
/// 必须在「退化未知枚举值」之后调用：被改成默认值、或被删掉的键就不算未知键了。
fn extract_node_extras(
    content: &serde_json::Value,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut out = HashMap::new();
    let Some(nodes) = content.get("nodes").and_then(|v| v.as_object()) else {
        return out;
    };
    let known = known_node_keys();
    for (id, raw) in nodes {
        let Some(obj) = raw.as_object() else { continue };
        let mut extra = serde_json::Map::new();
        for (key, value) in obj {
            if !known.contains(key) {
                extra.insert(key.clone(), value.clone());
            }
        }
        if !extra.is_empty() {
            out.insert(id.clone(), extra);
        }
    }
    out
}

/// 把节点级未知键并回 `content.json` 的 Value。
///
/// 注意：过一遍 `Value` 会让**键按字母序重排**（`serde_json::Map` 默认是 BTreeMap），
/// 所以只在真有节点级未知键时才走这条路；普通文档仍是结构体直接序列化，字节形状与旧版一致。
fn merge_node_extras(
    content: &mut serde_json::Value,
    extras: &HashMap<String, serde_json::Map<String, serde_json::Value>>,
) {
    let Some(nodes) = content.get_mut("nodes").and_then(|v| v.as_object_mut()) else {
        return;
    };
    for (id, extra) in extras {
        let Some(obj) = nodes.get_mut(id).and_then(|v| v.as_object_mut()) else {
            // 节点已经删了：挂在上面的未知键跟着一起走，不往别的节点上搬。
            continue;
        };
        for (key, value) in extra {
            obj.insert(key.clone(), value.clone());
        }
    }
}

/// 读侧宽松：把本版本不认识的**枚举取值**退化成语义上最安全的默认，而不是让整份文档打不开。
///
/// `TaskState` / `ArrowHead` 的枚举本体在 `mw-core`（本包不能加 `#[serde(other)]`），
/// 所以退化这一步放在反序列化之前、对 `content.json` 的 Value 做。
fn normalize_unknown_enums(content: &mut serde_json::Value) {
    let Some(nodes) = content.get_mut("nodes").and_then(|v| v.as_object_mut()) else {
        return;
    };
    for node in nodes.values_mut() {
        let Some(obj) = node.as_object_mut() else {
            continue;
        };
        // 关系线端点箭头：不认识的取值退回 forward（= 今天画的样子，与老文件缺这个键同一条路）。
        if let Some(rels) = obj.get_mut("relationships").and_then(|v| v.as_array_mut()) {
            for rel in rels.iter_mut() {
                let Some(rel) = rel.as_object_mut() else {
                    continue;
                };
                if let Some(head) = rel.get_mut("head") {
                    if !matches!(head.as_str(), Some("off" | "forward" | "backward" | "both")) {
                        *head = serde_json::Value::from("forward");
                    }
                }
            }
        }
        // 任务状态：不认识的取值按「没设置」处理。旧版本编不出「另一个状态」，
        // 编一个「未开始」出来等于替用户改了内容；删键与老文件缺这个键是同一条路径。
        if let Some(task) = obj.get("task") {
            if !matches!(task.as_str(), Some("todo" | "doing" | "done")) {
                obj.remove("task");
            }
        }
    }
}

/// 保存文档（原子流程 + 滚动备份 `*.zmind.bak`）。
pub fn save(doc: &Document, path: &Path) -> Result<(), FormatError> {
    // 本版本不认识的键：只有当这份文档确实是从目标文件读来的，才敢原样写回去。
    let extras = extras_for_save(&doc.id, path);
    let file = FileDoc {
        schema: FORMAT_VERSION,
        root_id: doc.root_id.clone(),
        nodes: doc.nodes.clone(),
        theme: doc.meta.theme.clone(),
        theme_accent: doc.meta.theme_accent.clone(),
        theme_overrides: doc.meta.theme_overrides.clone(),
        layout: doc.meta.layout.clone(),
        cards: doc.cards.clone(),
        groups: doc.groups.clone(),
        // 概念图写出专属块（`docs/33` §3）：`nodes` 照写一份当兼容镜像。
        concept_map: if doc.meta.layout.kind == "conceptMap" {
            Some(concept_map::from_nodes(
                &doc.nodes,
                doc.meta.layout.focus_question.clone(),
                doc.groups.clone(),
            ))
        } else {
            None
        },
        extra: extras.doc.clone(),
    };
    // 没有节点级未知键就走结构体直接序列化（与旧版字节形状一致）；
    // 只有真有节点级未知键时才过一遍 Value 把键补回去。
    let content = if extras.nodes.is_empty() {
        serde_json::to_vec(&file)?
    } else {
        let mut value = serde_json::to_value(&file)?;
        merge_node_extras(&mut value, &extras.nodes);
        serde_json::to_vec(&value)?
    };

    let created_at = existing_created_at(path).unwrap_or_else(mw_core::now_ms);
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        app: APP_ID.to_string(),
        doc_id: doc.id.clone(),
        title: doc.meta.title.clone(),
        created_at,
        modified_at: mw_core::now_ms(),
        node_count: doc.node_count() as u64,
        note_count: doc.cards.len() as u64,
        asset_count: doc.assets.len() as u64,
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)?;

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", stored)?;
        zip.write_all(MIME.as_bytes())?;
        let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json", deflate)?;
        zip.write_all(&manifest_json)?;
        zip.start_file("content.json", deflate)?;
        zip.write_all(&content)?;
        // 卡片正文另外写成独立 Markdown：解开 .zmind 就能直接读，不依赖应用。
        for card in &doc.cards {
            zip.start_file(note_entry(card), deflate)?;
            zip.write_all(card_markdown(card).as_bytes())?;
        }
        // 资源：哈希命名，同一张图只写一条。
        for (hash, bytes) in doc.assets.iter() {
            zip.start_file(format!("assets/{hash}"), deflate)?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
    }
    write_atomic(path, &buf.into_inner())
}

// ---------------------------------------------------------------------------
// 保真槽（开放格式 .xmind / .cxl 共享，见 docs/34 §2）
// ---------------------------------------------------------------------------

/// 开放格式里的保真槽：完整文档 + 标题/标识/创建时间。
///
/// `.zmind` 的标题在 `manifest.json` 里、内容在 `content.json` 里；开放格式
/// （`.xmind`/`.cxl`）没有独立 manifest，故把标题/标识一并塞进保真槽，自包含。
/// `content` 与 `.zmind` 的 `content.json` 同构（同一个 `FileDoc`），保真度一致。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct FidelityDoc {
    doc_id: String,
    title: String,
    created_at: i64,
    content: FileDoc,
}

/// 把 `Document` 收成 `FileDoc`（与 `.zmind` 的 `content.json` 同构）。
/// 不做「未知键保留」——那是 `.zmind` 源文件对账的活，保真槽只保当前已知模型。
fn build_file_doc(doc: &Document) -> FileDoc {
    FileDoc {
        schema: FORMAT_VERSION,
        root_id: doc.root_id.clone(),
        nodes: doc.nodes.clone(),
        theme: doc.meta.theme.clone(),
        theme_accent: doc.meta.theme_accent.clone(),
        theme_overrides: doc.meta.theme_overrides.clone(),
        layout: doc.meta.layout.clone(),
        cards: doc.cards.clone(),
        groups: doc.groups.clone(),
        // 概念图写出专属块（`docs/33` §3）；思维导图不写。
        concept_map: if doc.meta.layout.kind == "conceptMap" {
            Some(concept_map::from_nodes(
                &doc.nodes,
                doc.meta.layout.focus_question.clone(),
                doc.groups.clone(),
            ))
        } else {
            None
        },
        extra: serde_json::Map::new(),
    }
}

/// `Document` → 保真槽 JSON 字节（`.xmind` 的 `mindweaver.json` / `.cxl` 的扩展属性）。
/// `created_at` 传 `None` 取当前时间。
pub fn doc_to_fidelity_json(
    doc: &Document,
    created_at: Option<i64>,
) -> Result<Vec<u8>, FormatError> {
    let fid = FidelityDoc {
        doc_id: doc.id.clone(),
        title: doc.meta.title.clone(),
        created_at: created_at.unwrap_or_else(mw_core::now_ms),
        content: build_file_doc(doc),
    };
    Ok(serde_json::to_vec(&fid)?)
}

/// `FileDoc` → `Document`（与 `.zmind` 加载的 `FileDoc → Document` 同构）。
/// `conceptMap` 块存在时它是权威（`docs/33` §3）。
fn doc_from_file_doc(file: &FileDoc, title: &str, doc_id: &str) -> Result<Document, FormatError> {
    let meta = DocMeta {
        title: title.to_string(),
        theme: file.theme.clone(),
        theme_accent: file.theme_accent.clone(),
        theme_overrides: file.theme_overrides.clone(),
        layout: file.layout.clone(),
    };
    let nodes = match &file.concept_map {
        Some(block) => concept_map::to_nodes(block),
        None => file.nodes.clone(),
    };
    Document::from_parts_full(
        Some(doc_id.to_string()),
        meta,
        file.root_id.clone(),
        nodes,
        file.cards.clone(),
        file.groups.clone(),
    )
    .map_err(|e| FormatError::Core(e.to_string()))
}

/// 保真槽 JSON 字节 → `Document`（无损往返）。开放格式加载时优先走这里。
pub fn doc_from_fidelity_json(bytes: &[u8]) -> Result<LoadedDoc, FormatError> {
    let mut v: serde_json::Value = serde_json::from_slice(bytes)?;
    normalize_unknown_enums(&mut v);
    let fid: FidelityDoc = serde_json::from_value(v)?;
    let doc = doc_from_file_doc(&fid.content, &fid.title, &fid.doc_id)?;
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        app: APP_ID.to_string(),
        doc_id: fid.doc_id,
        title: fid.title,
        created_at: fid.created_at,
        modified_at: mw_core::now_ms(),
        node_count: doc.node_count() as u64,
        note_count: doc.cards.len() as u64,
        asset_count: doc.assets.len() as u64,
    };
    Ok(LoadedDoc { doc, manifest })
}

/// 概念图节点表 → 专属块（公开包装，开放格式 `.cxl` 用）。
pub fn concept_map_block(
    nodes: &HashMap<String, Node>,
    focus_question: Option<String>,
    groups: Vec<mw_core::model::ConceptGroup>,
) -> ConceptMapBlock {
    concept_map::from_nodes(nodes, focus_question, groups)
}

/// 专属块 → 概念图节点表（公开包装）。
pub fn concept_to_nodes(block: &ConceptMapBlock) -> HashMap<String, Node> {
    concept_map::to_nodes(block)
}

/// 卡片在 ZIP 里的条目名。id 只保留 ASCII 字母数字/`-_`，防止 `../` 之类的路径穿越。
fn note_entry(card: &NoteCard) -> String {
    let safe: String = card
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("notes/{}.md", if safe.is_empty() { "card" } else { &safe })
}

fn yaml_str(s: &str) -> String {
    // JSON 字符串是合法的 YAML 双引号字符串，省得自己实现转义。
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// 把卡片导出成一个 Markdown 文件夹：`<安全 id>.md`，内容与 `.zmind` 内嵌编码一致。
/// 返回写入数量。不删除目标目录里已有的其它文件。
pub fn export_cards_dir(cards: &[NoteCard], dir: &Path) -> Result<usize, FormatError> {
    std::fs::create_dir_all(dir)?;
    let mut written = 0;
    for card in cards {
        card.validate().map_err(FormatError::Invalid)?;
        let name = note_entry(card)
            .rsplit('/')
            .next()
            .unwrap_or("card.md")
            .to_string();
        std::fs::write(dir.join(name), card_markdown(card))?;
        written += 1;
    }
    Ok(written)
}

/// 从一个 Markdown 文件夹读回卡片。坏文件只记文件名，不中断。
#[derive(Clone, Debug, Default)]
pub struct CardDirImport {
    pub cards: Vec<NoteCard>,
    pub skipped: Vec<String>,
}

pub fn import_cards_dir(dir: &Path) -> Result<CardDirImport, FormatError> {
    let mut out = CardDirImport::default();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => match card_from_markdown(&text) {
                Ok(card) => out.cards.push(card),
                Err(_) => out.skipped.push(label),
            },
            Err(_) => out.skipped.push(label),
        }
    }
    out.cards.sort_by(|a, b| a.id.cmp(&b.id));
    out.skipped.sort();
    Ok(out)
}

/// 卡片 → 独立 Markdown（YAML front matter + 正文）。解压 `.zmind` 即可阅读，
/// 全局笔记库也复用这一份编码，避免 `.zmind` 内与库里的格式漂移。
pub fn card_markdown(card: &NoteCard) -> String {
    let tags = serde_json::to_string(&card.tags).unwrap_or_else(|_| "[]".into());
    let node = card
        .source_node
        .as_deref()
        .map(yaml_str)
        .unwrap_or_else(|| "null".into());
    // 归档只在真归档时写键：未归档的卡片字节不变，老库/老夹具一张都不漂。
    let archived = if card.archived {
        "archived: true\n"
    } else {
        ""
    };
    format!(
        "---\nid: {}\ntitle: {}\ntags: {}\nsourceDoc: {}\nsourceNode: {}\n{}createdAt: {}\nupdatedAt: {}\n---\n\n{}",
        yaml_str(&card.id),
        yaml_str(&card.title),
        tags,
        yaml_str(&card.source_doc),
        node,
        archived,
        card.created_at,
        card.updated_at,
        card.body
    )
}

fn scalar(v: &str) -> String {
    let v = v.trim();
    serde_json::from_str::<String>(v).unwrap_or_else(|_| v.trim_matches('"').to_string())
}

/// 读回 `card_markdown`。只支持我们写出的那几行 front matter，不做通用 YAML。
pub fn card_from_markdown(text: &str) -> Result<NoteCard, FormatError> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| FormatError::Invalid("卡片缺少 front matter".into()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| FormatError::Invalid("卡片 front matter 没有结束行".into()))?;
    let (head, tail) = rest.split_at(end);
    let body = tail
        .strip_prefix("\n---")
        .unwrap_or("")
        .strip_prefix('\n')
        .unwrap_or("")
        .strip_prefix('\n')
        .unwrap_or("");

    let mut map: HashMap<&str, &str> = HashMap::new();
    for line in head.lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim(), v.trim());
        }
    }
    let field = |k: &str| map.get(k).map(|v| scalar(v)).unwrap_or_default();
    let tags = map
        .get("tags")
        .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok())
        .unwrap_or_default();
    let node = map
        .get("sourceNode")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty() && *v != "null")
        .map(scalar);
    let card = NoteCard {
        id: field("id"),
        title: field("title"),
        body: body.to_string(),
        tags,
        source_doc: field("sourceDoc"),
        source_node: node,
        created_at: map
            .get("createdAt")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0),
        updated_at: map
            .get("updatedAt")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0),
        // 缺键即未归档：老库读回来行为不变，不需要迁移。
        archived: map
            .get("archived")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
    };
    card.validate().map_err(FormatError::Invalid)?;
    Ok(card)
}

/// 保存的落盘全过程：写 tmp（并 fsync）→ 正本转 .bak → 改名 → 目录 fsync。
///
/// 每一步失败都要么「什么都没变」，要么把 `.bak` 改回原名——**不许出现没有正本的状态**。
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FormatError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("document.zmind");
    // 上次崩溃留下的 tmp：尽力清掉（只清足够旧的，见 `sweep_stale_tmp`）。
    sweep_stale_tmp(dir, file_name);

    // 1) 内容先落盘：这一步之后 tmp 已经是一份完整文件。
    let tmp = tmp_path(dir, file_name);
    if let Err(e) = write_synced(&tmp, bytes) {
        // 半截 tmp 不能留：它长得就像一份真文件。
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    // 2) 正本转 .bak。这一步失败就放弃本次保存——正本还在原地，
    //    绝不能吞掉错误继续往下 rename（那会把正本覆盖成半截状态）。
    let bak = dir.join(format!("{file_name}.bak"));
    if path.exists() {
        let _ = std::fs::remove_file(&bak);
        if let Err(e) = std::fs::rename(path, &bak) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
    }

    // 3) tmp → 正本。失败时把 .bak 改回原名，否则用户看到的是「文件不见了」。
    if let Err(e) = commit(&tmp, path) {
        let _ = std::fs::rename(&bak, path);
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    // 4) 目录也落盘（rename 本身要落盘，否则掉电后可能回到「只有 tmp」）。
    sync_dir(dir);
    Ok(())
}

/// 同目录临时文件名：`.{文件名}.{pid}.{序号}.{纳秒}.tmp`。
///
/// 带 pid + 进程内序号 + 纳秒：两个写者（自动保存与手动保存、或两个进程）不会撞到同一个
/// tmp 上——撞上去的后果是把对方的半成品改名成正本。
fn tmp_path(dir: &Path, file_name: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    dir.join(format!(
        ".{file_name}.{}.{seq}.{nanos}.tmp",
        std::process::id()
    ))
}

/// 陈旧 tmp 的判定阈值：一小时前建的 tmp 一定是上次崩溃留下的。
const STALE_TMP_AGE: Duration = Duration::from_secs(60 * 60);

/// 清掉同目录里上次保存崩溃留下的 `.{文件名}.*.tmp`。**尽力而为**：任何错误都忽略
/// （磁盘满、权限、文件被占用都不该让一次保存失败）。
///
/// 只删「足够旧」的：删掉另一个写者刚建的 tmp，等于亲手制造它刚要消灭的那种故障。
fn sweep_stale_tmp(dir: &Path, file_name: &str) {
    let prefix = format!(".{file_name}.");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(&prefix) || !name.ends_with(".tmp") {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| mtime.elapsed().ok())
            .is_some_and(|age| age > STALE_TMP_AGE);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 写 tmp 并落盘。**内容没落盘就返回错误**，不许往下走——否则掉电后 rename 可能先落盘，
/// 正本指向一份内容为空的文件，恰好破坏「任一时点至少一份完整文件」。
fn write_synced(tmp: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(tmp)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// tmp → 正本的最后一步。
///
/// 单独抽出来是为了让测试能注入失败：Windows 上没有可靠办法用「只读目标」让 rename 失败，
/// 而「`.bak` 回滚」这条分支必须自己有测试。
fn commit(tmp: &Path, path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_COMMIT.with(|flag| flag.get()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "测试注入：改名失败",
        ));
    }
    std::fs::rename(tmp, path)
}

// 测试注入点：让本线程下一次「tmp → 正本」的 rename 失败。
#[cfg(test)]
thread_local! {
    static FAIL_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 让目录也落盘（rename 本身要落盘）。
///
/// Windows 上 `File::open(目录)` 会失败（拿不到目录句柄），拿不到句柄就没有 fsync 可用；
/// 失败一律忽略——**不能因为「同步目录做不到」就让一次成功的保存报错**。
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// 读取文档；包含版本守卫、解压体积守卫与路径穿越检查。
pub fn load(path: &Path) -> Result<LoadedDoc, FormatError> {
    let bytes = std::fs::read(path)?;
    // 带上源路径：未知键账本要能区分「同一份文档的两份副本」。
    load_from_bytes_with_source(&bytes, Some(path))
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<LoadedDoc, FormatError> {
    // 没有源文件：账本照记，但保存时对不上路径就不写回（见 `extras_for_save`）。
    load_from_bytes_with_source(bytes, None)
}

fn load_from_bytes_with_source(
    bytes: &[u8],
    source: Option<&Path>,
) -> Result<LoadedDoc, FormatError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > MAX_ENTRIES {
        return Err(FormatError::Invalid("条目数量超过安全上限".into()));
    }
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.enclosed_name().is_none() {
            return Err(FormatError::Invalid("存在非法条目路径".into()));
        }
        let size = entry.size();
        if size > MAX_ENTRY {
            return Err(FormatError::Invalid("单条目体积超过安全上限".into()));
        }
        total += size;
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err(FormatError::Invalid("解压内容超过安全上限".into()));
        }
    }

    let mut manifest_bytes = Vec::new();
    {
        let mut f = archive
            .by_name("manifest.json")
            .map_err(|_| FormatError::Invalid("缺少 manifest.json".into()))?;
        f.read_to_end(&mut manifest_bytes)?;
    }
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.format_version > FORMAT_VERSION {
        return Err(FormatError::TooNew(manifest.format_version));
    }

    let mut content_bytes = Vec::new();
    {
        let mut f = archive
            .by_name("content.json")
            .map_err(|_| FormatError::Invalid("缺少 content.json".into()))?;
        f.read_to_end(&mut content_bytes)?;
    }
    let mut content_value: serde_json::Value = serde_json::from_slice(&content_bytes)?;
    // 读侧宽松：一个未知枚举取值不该让整份文档打不开。
    normalize_unknown_enums(&mut content_value);
    // 先把「本版本不认识的键」拣出来再反序列化（节点级得看原始 JSON），
    // 见得一次就记一次账，再保存时原样写回去。
    let node_extras = extract_node_extras(&content_value);
    let file: FileDoc = serde_json::from_value(content_value)?;
    remember_extras(
        &manifest.doc_id,
        source,
        DocExtras {
            doc: file.extra.clone(),
            nodes: node_extras,
        },
    );

    let meta = DocMeta {
        title: manifest.title.clone(),
        theme: file.theme.clone(),
        theme_accent: file.theme_accent.clone(),
        theme_overrides: file.theme_overrides.clone(),
        layout: file.layout.clone(),
    };
    // `conceptMap` 块存在时它是权威（`docs/33` §3）：`nodes` 被忽略。
    // 缺块回落到旧形状，老文件一个字节都不用改。
    let nodes = match &file.concept_map {
        Some(block) => concept_map::to_nodes(block),
        None => file.nodes.clone(),
    };
    let mut doc = Document::from_parts_full(
        Some(manifest.doc_id.clone()),
        meta,
        file.root_id.clone(),
        nodes,
        file.cards.clone(),
        file.groups.clone(),
    )
    .map_err(|e| FormatError::Core(e.to_string()))?;
    // 资源包：缺条目不报错（节点图片引用会降级成占位）。
    for i in 0..archive.len() {
        let Ok(mut f) = archive.by_index(i) else {
            continue;
        };
        let Ok(name) = f.name() else {
            continue;
        };
        let name = name.to_string();
        let Some(hash) = name.strip_prefix("assets/") else {
            continue;
        };
        if hash.is_empty() || f.size() > MAX_ENTRY {
            continue;
        }
        let mut bytes = Vec::new();
        if f.read_to_end(&mut bytes).is_ok() {
            doc.assets.put(hash.to_string(), bytes);
        }
    }
    Ok(LoadedDoc { doc, manifest })
}

/// 只读取元数据（恢复提示 / 最近文件列表用）。
pub fn peek_manifest(path: &Path) -> Option<Manifest> {
    let bytes = std::fs::read(path).ok()?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut buf = Vec::new();
    let mut f = archive.by_name("manifest.json").ok()?;
    f.read_to_end(&mut buf).ok()?;
    serde_json::from_slice::<Manifest>(&buf).ok()
}

fn existing_created_at(path: &Path) -> Option<i64> {
    peek_manifest(path).map(|m| m.created_at).filter(|v| *v > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mw_core::{new_id, StyleOverride};

    fn sample_doc() -> Document {
        let mut d = Document::new("示例文档");
        d.meta.theme = "mint".into();
        d.meta.theme_accent = Some("#e5484d".into());
        d.meta.layout.sibling_gap = 30.0;
        let root = d.root_id.clone();

        let mut a = Node::new(new_id(), Some(root.clone()), "市场调研");
        a.labels = vec!["重点".into(), "Q3".into()];
        let b = Node::new(new_id(), Some(root.clone()), "里程碑\n第二阶段");
        let mut a2 = Node::new(new_id(), Some(a.id.clone()), "竞品");
        a2.note = Some("重点看 XMind".into());
        a2.style = Some(StyleOverride {
            fill: Some("#E8F1FF".into()),
            bold: Some(true),
            ..Default::default()
        });

        let a_id = a.id.clone();
        let b_id = b.id.clone();
        let a2_id = a2.id.clone();
        d.nodes.insert(a_id.clone(), a);
        d.nodes.insert(b_id.clone(), b);
        d.nodes.insert(a2_id.clone(), a2);
        d.nodes.get_mut(&root).unwrap().children = vec![a_id.clone(), b_id.clone()];
        d.nodes.get_mut(&a_id).unwrap().children = vec![a2_id.clone()];

        // 高级元素：关系线 / 边界 / 概要 / 自由主题
        d.nodes
            .get_mut(&a_id)
            .unwrap()
            .relationships
            .push(mw_core::Relationship {
                id: new_id(),
                to: a2_id.clone(),
                label: Some("包含".into()),
                color: None,
                head: Default::default(),
                ..Default::default()
            });
        d.nodes
            .get_mut(&root)
            .unwrap()
            .boundaries
            .push(mw_core::Boundary {
                id: new_id(),
                targets: vec![a_id.clone()],
                color: None,
            });
        d.nodes
            .get_mut(&root)
            .unwrap()
            .summaries
            .push(mw_core::Summary {
                id: new_id(),
                targets: vec![a_id.clone(), b_id],
                text: "上半年".into(),
            });
        let mut floater = Node::new(new_id(), None, "浮动便签");
        floater.pos = Some(mw_core::Position { x: 320.0, y: -60.0 });
        floater.labels = vec!["临时".into()];
        d.nodes.insert(floater.id.clone(), floater);

        d.validate().unwrap();
        d
    }

    #[test]
    fn card_markdown_round_trips_and_rejects_garbage() {
        let mut c = NoteCard::new(
            "doc-1",
            Some("node-1".into()),
            "标题: \"带引号\"",
            "# 一\n\n正文",
            11,
        );
        c.tags = vec!["Rust".into(), "知识 管理".into()];
        c.updated_at = 12;
        let md = card_markdown(&c);
        assert!(
            !md.contains("archived"),
            "未归档的卡片不该多出 archived 键，否则老库字节全漂"
        );
        let back = card_from_markdown(&md).unwrap();
        assert_eq!(back, c, "front matter + 正文必须逐字段往返");
        assert!(!back.archived);

        c.archived = true;
        let md = card_markdown(&c);
        let back = card_from_markdown(&md).unwrap();
        assert_eq!(back, c, "归档标记必须往返");

        assert!(card_from_markdown("没有 front matter").is_err());
        assert!(
            card_from_markdown("---\nid: \"\"\n---\n正文").is_err(),
            "空 id 要拒绝"
        );
    }

    /// `.zmind` 内嵌卡片走 JSON：老文件缺 `archived` 键要读成未归档，
    /// 新文件只在真归档时写键（`docs/32` 的字段例外，不升 `FORMAT_VERSION`）。
    #[test]
    fn embedded_card_json_treats_missing_archived_as_false() {
        let legacy = r#"{"id":"a","title":"T","body":"","tags":[],"sourceDoc":"d","sourceNode":null,"createdAt":1,"updatedAt":2}"#;
        let back: NoteCard = serde_json::from_str(legacy).expect("缺键的老记录要能读");
        assert!(!back.archived, "缺 archived 键必须当未归档，不能当归档");
        let fresh = serde_json::to_string(&back).unwrap();
        assert!(
            !fresh.contains("archived"),
            "未归档时不该多出这个键，否则老 .zmind 字节全漂：{fresh}"
        );
        let mut arch = back;
        arch.archived = true;
        assert!(serde_json::to_string(&arch)
            .unwrap()
            .contains("\"archived\":true"));
    }

    #[test]
    fn card_folders_export_import_byte_exact_and_keep_tags() {
        let dir = scratch("mw-carddir");
        let dir2 = scratch("mw-carddir2");
        let mut a = NoteCard::new("doc", Some("n1".into()), "甲", "# 正文一", 1);
        a.tags = vec!["知识".into(), "Rust".into()];
        let mut b = NoteCard::new("doc", None, "乙", "正文二", 2);
        b.tags = vec!["每日".into()];
        let cards = vec![a.clone(), b.clone()];

        assert_eq!(export_cards_dir(&cards, &dir).unwrap(), 2);
        let back = import_cards_dir(&dir).unwrap();
        assert!(back.skipped.is_empty());
        let mut expect = cards;
        expect.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(back.cards, expect, "标题/正文/标签/时间逐字段往返");

        // 再导出一次，文件字节必须完全一致（front matter 确定性）。
        assert_eq!(export_cards_dir(&back.cards, &dir2).unwrap(), 2);
        for name in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = name.file_name();
            let x = std::fs::read(dir.join(&name)).unwrap();
            let y = std::fs::read(dir2.join(&name)).unwrap();
            assert_eq!(x, y, "往返后 {} 字节不一致", name.to_string_lossy());
        }
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[test]
    fn card_folder_import_skips_bad_files() {
        let dir = scratch("mw-cardbad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("good.md"),
            card_markdown(&NoteCard::new("d", None, "T", "B", 0)),
        )
        .unwrap();
        std::fs::write(dir.join("bad.md"), "不是卡片").unwrap();
        std::fs::write(dir.join("ignore.txt"), "x").unwrap();
        let out = import_cards_dir(&dir).unwrap();
        assert_eq!(out.cards.len(), 1);
        assert_eq!(out.skipped, vec!["bad.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cards_round_trip_and_become_readable_markdown_entries() {
        let mut d = sample_doc();
        let root = d.root_id.clone();
        d.cards.push(NoteCard::new(
            d.id.clone(),
            Some(root.clone()),
            "概念卡",
            "# 标题\n\n正文 **重点**",
            7,
        ));
        d.nodes.get_mut(&root).unwrap().card_id = Some(d.cards[0].id.clone());
        let path = temp_path("cards.zmind");
        save(&d, &path).unwrap();

        // ZIP 里必须有一份可直接阅读的 Markdown，而不是只有 JSON。
        let bytes = std::fs::read(&path).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let entry = note_entry(&d.cards[0]);
        let mut md = String::new();
        archive
            .by_name(&entry)
            .unwrap()
            .read_to_string(&mut md)
            .unwrap();
        assert!(
            md.contains("# 标题") && md.contains("正文 **重点**"),
            "{md}"
        );
        assert!(md.contains("title: \"概念卡\""), "{md}");
        assert!(md.starts_with("---\n"), "front matter 必须在最前：{md}");

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.doc.cards, d.cards, "卡片要逐字段往返");
        assert_eq!(
            loaded.doc.card_for_node(&root).map(|c| c.id.clone()),
            Some(d.cards[0].id.clone()),
            "node.cardId 也要往返"
        );
        assert_eq!(loaded.manifest.note_count, 1);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 测试落地目录：**仓库内 target/tmp 优先**。
    ///
    /// 不用 std::env::temp_dir() 打头：本机用户 Temp 对测试进程偶发拒写
    /// （os error 5 拒绝访问），会让下面 8 条写盘用例整体假失败——
    /// demo_notes.rs 的夹具目录踩过同一个坑。target/tmp 一定可写，也便于清理。
    fn scratch(prefix: &str) -> std::path::PathBuf {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/tests");
        let base = if std::fs::create_dir_all(&base).is_ok() {
            base
        } else {
            std::env::temp_dir()
        };
        let dir = base.join(format!("{}-{}", prefix, new_id()));
        std::fs::create_dir_all(&dir).expect("测试临时目录必须能创建");
        dir
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        scratch("mw-format-test").join(name)
    }

    #[test]
    fn assets_are_deduped_saved_and_bad_refs_load() {
        let mut doc = sample_doc();
        let a = doc.assets.insert(vec![9, 8, 7]);
        let b = doc.assets.insert(vec![9, 8, 7]);
        assert_eq!(a, b);
        let root = doc.root_id.clone();
        doc.apply(&mw_core::Command::SetImage {
            id: root,
            image: Some(format!("asset:{a}")),
        })
        .unwrap();
        let path = temp_path("资源.zmind");
        save(&doc, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.doc.assets.len(), 1);
        assert_eq!(loaded.doc.assets.get(&a), Some(&[9u8, 8, 7][..]));
        assert_eq!(loaded.manifest.asset_count, 1);

        // 节点指向不存在的资源：加载不报错，渲染层降级成占位。
        let mut broken = loaded.doc;
        let root = broken.root_id.clone();
        broken
            .apply(&mw_core::Command::SetImage {
                id: root,
                image: Some("asset:deadbeef".into()),
            })
            .unwrap();
        let path = temp_path("坏资源.zmind");
        save(&broken, &path).unwrap();
        assert!(load(&path).is_ok());
    }

    /// 概念图不新增字段、不升版本：`kind` 字符串 + `pos` + `relationships` 原样往返。
    #[test]
    fn concept_map_roundtrips_positions_and_relationships() {
        let mut doc = mw_core::Document::new("概念图往返");
        doc.apply(&mw_core::Command::SetLayout {
            layout: mw_core::LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        })
        .unwrap();
        let root = doc.root_id.clone();
        doc.apply(&mw_core::Command::SetText {
            id: root.clone(),
            text: "产品定位".into(),
        })
        .unwrap();
        doc.apply(&mw_core::Command::AddFloatingTopic {
            text: "用户痛点".into(),
            x: -132.0,
            y: 47.5,
        })
        .unwrap();
        let float = doc
            .nodes
            .values()
            .find(|n| n.text == "用户痛点")
            .expect("自由概念要建出来")
            .id
            .clone();
        doc.apply(&mw_core::Command::AddRelationship {
            from: root.clone(),
            to: float.clone(),
            label: Some("解决".into()),
        })
        .unwrap();

        let path = temp_path("概念图.zmind");
        save(&doc, &path).unwrap();
        let loaded = load(&path).unwrap();
        let back = &loaded.doc;

        assert_eq!(back.meta.layout.kind, "conceptMap", "文档类型要原样回来");
        let f = &back.nodes[&float];
        assert_eq!(f.parent_id, None, "概念仍是浮动节点");
        let pos = f.pos.as_ref().expect("坐标要保留");
        assert_eq!((pos.x, pos.y), (-132.0, 47.5), "坐标逐字段相等");
        let rel = back.nodes[&root]
            .relationships
            .iter()
            .find(|r| r.to == float)
            .expect("关系线要保留");
        assert_eq!(rel.label.as_deref(), Some("解决"), "链接短语要保留");
        assert_eq!(rel.color, None, "未指定颜色时保持 None，不偷偷补默认色");
        assert!(back.validate().is_ok(), "概念图结构要合法");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn roundtrip_preserves_nodes_and_meta() {
        let doc = sample_doc();
        let path = temp_path("示例.zmind");
        save(&doc, &path).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.doc.root_id, doc.root_id);
        assert_eq!(loaded.doc.meta, doc.meta);
        assert_eq!(loaded.doc.node_count(), doc.node_count());
        for (id, n) in &doc.nodes {
            assert_eq!(&loaded.doc.nodes[id], n, "节点 {id} 不一致");
        }
        assert_eq!(loaded.manifest.title, "示例文档");
        assert_eq!(loaded.manifest.format_version, FORMAT_VERSION);
        assert_eq!(loaded.manifest.node_count as usize, doc.node_count());
        assert!(loaded.doc.validate().is_ok());

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn second_save_creates_backup_and_preserves_created_at() {
        let doc = sample_doc();
        let path = temp_path("doc.zmind");
        save(&doc, &path).unwrap();
        let first = peek_manifest(&path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        save(&doc, &path).unwrap();
        let second = peek_manifest(&path).unwrap();
        assert_eq!(first.created_at, second.created_at);
        assert!(path.with_file_name("doc.zmind.bak").exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 读出容器里真正写出去的 `content.json`——要验的是字节形状，不是内存里的结构体。
    fn content_json_of(path: &Path) -> String {
        let file = std::fs::File::open(path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut s = String::new();
        zip.by_name("content.json")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    }

    /// 读出容器里真正写出去的 `content.json` 的**结构**（不看键序，只看键值）。
    fn content_value_of(path: &Path) -> serde_json::Value {
        serde_json::from_str(&content_json_of(path)).unwrap()
    }

    fn file_json(doc: &Document) -> String {
        let file = FileDoc {
            schema: FORMAT_VERSION,
            root_id: doc.root_id.clone(),
            nodes: doc.nodes.clone(),
            theme: doc.meta.theme.clone(),
            theme_accent: doc.meta.theme_accent.clone(),
            theme_overrides: doc.meta.theme_overrides.clone(),
            layout: doc.meta.layout.clone(),
            cards: doc.cards.clone(),
            groups: doc.groups.clone(),
            // 与写盘同一口径（`docs/33` §3）：概念图写出专属块。
            concept_map: if doc.meta.layout.kind == "conceptMap" {
                Some(concept_map::from_nodes(
                    &doc.nodes,
                    doc.meta.layout.focus_question.clone(),
                    doc.groups.clone(),
                ))
            } else {
                None
            },
            extra: serde_json::Map::new(),
        };
        serde_json::to_string(&file).unwrap()
    }

    #[test]
    fn the_accent_key_is_written_only_when_one_was_set() {
        let mut unset = sample_doc();
        unset.meta.theme_accent = None;
        let (p_unset, p_set) = (temp_path("unset.zmind"), temp_path("set.zmind"));
        save(&unset, &p_unset).unwrap();
        save(&sample_doc(), &p_set).unwrap();
        // 读侧对未知键宽容，写侧必须严格：没设置就不该多出一个键，
        // 否则「未设置」和「设成了主题默认色」这两种状态在文件里再也分不开。
        assert!(
            !content_json_of(&p_unset).contains("themeAccent"),
            "未设置时不该出现这个键"
        );
        assert!(
            content_json_of(&p_set).contains(r##""themeAccent":"#e5484d""##),
            "设置了就该原样写进去"
        );
        let _ = std::fs::remove_dir_all(p_unset.parent().unwrap());
        let _ = std::fs::remove_dir_all(p_set.parent().unwrap());
    }

    #[test]
    fn groups_are_written_only_when_present() {
        let mut d = sample_doc();
        // 没有分组：整个键都不写（老文档的 content.json 保持原样）。
        assert!(!file_json(&d).contains("\"groups\""));
        d.groups.push(mw_core::model::ConceptGroup {
            id: "g1".into(),
            label: "分组 1".into(),
            members: vec!["a".into(), "b".into()],
            color: None,
        });
        let json = file_json(&d);
        assert!(json.contains("\"groups\""), "{json}");
        assert!(json.contains("\"members\""), "{json}");
        assert!(json.contains("分组 1"), "{json}");
    }

    #[test]
    fn relationship_line_style_uses_defaults_and_writes_nothing_when_unset() {
        use mw_core::model::{ArrowHead, Relationship};
        // 默认（实线 / 主题线宽 / 无注释 / 前向箭头）：四个键一个都不该落盘。
        let plain = Relationship {
            id: "r1".into(),
            to: "n2".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&plain).unwrap();
        assert!(!json.contains("dashed"), "{json}");
        assert!(!json.contains("width"), "{json}");
        assert!(!json.contains("note"), "{json}");
        assert!(!json.contains("head"), "默认箭头也不落盘：{json}");

        // 旧文件缺这三个键 → 读出来就是默认值（不升 FORMAT_VERSION 的依据）。
        let back: Relationship = serde_json::from_str(r#"{"id":"r1","to":"n2"}"#).unwrap();
        assert!(!back.dashed);
        assert_eq!(back.width, None);
        assert_eq!(back.note, None);
        assert_eq!(back.head, ArrowHead::Forward);
        assert!((back.width_css() - 1.5).abs() < 1e-6, "没设线宽就跟随主题");

        // 设了就原样写进去。
        let styled = Relationship {
            dashed: true,
            width: Some(2.5),
            note: Some("暂定，待验证".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&styled).unwrap();
        assert!(json.contains(r#""dashed":true"#), "{json}");
        assert!(json.contains(r#""width":2.5"#), "{json}");
        assert!(json.contains("暂定，待验证"), "{json}");
    }

    /// SE-13：三个新样式键（`weight` / `align` / `radius`）**可以不出现**——
    /// 旧文件缺这三个键读出来是老行为（左对齐 / 形状自带圆角 / `bold ? 600 : 400`），
    /// 所以不升 `FORMAT_VERSION`，也不需要迁移。
    ///
    /// 落盘形态与既有兄弟字段一致：未设置的键写成 `null`（`style` 整体是 `Option`，
    /// 没有覆盖时写 `"style":null`）。`null` 读回来还是 `None`，所以对读侧而言
    /// 「缺键」与「写 null」是同一件事，旧文件不需要迁移。
    #[test]
    fn the_new_style_keys_are_optional_and_default_to_the_old_look() {
        use mw_core::model::StyleOverride;
        let plain = StyleOverride {
            fill: Some("#ff0000".into()),
            ..Default::default()
        };
        // 旧文件（完全没有这三个键）→ 读出来是 None，也就是老行为。
        let back: StyleOverride =
            serde_json::from_str(r##"{"fill":"#ff0000","fontSize":18,"shape":"rounded"}"##)
                .unwrap();
        assert_eq!(back.weight, None);
        assert_eq!(back.align, None);
        assert_eq!(back.radius, None);

        // 设了就原样写进去，再读回逐字段相等。
        let styled = StyleOverride {
            weight: Some(600),
            align: Some("center".into()),
            radius: Some(14.0),
            shape: Some("rounded".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&styled).unwrap();
        assert!(json.contains(r#""weight":600"#), "{json}");
        assert!(json.contains(r#""align":"center""#), "{json}");
        assert!(json.contains(r#""radius":14"#), "{json}");
        let back: StyleOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(back, styled, "新键必须逐字段往返");

        // 只设了一个兄弟字段时，三个新键一个都不该落盘——旧文档的样式对象字节不变。
        let json = serde_json::to_string(&plain).unwrap();
        for key in ["weight", "align", "radius"] {
            assert!(!json.contains(key), "缺省不该写 {key}：{json}");
        }
        // 没覆盖样式的节点：`style` 仍是 `null`（既有形态，不动）。
        let bare = mw_core::Node::new("n1".to_string(), None, "甲".to_string());
        let json = serde_json::to_string(&bare).unwrap();
        assert!(json.contains("\"style\":null"), "没样式写成 null：{json}");
    }

    #[test]
    fn theme_overrides_are_written_only_when_set() {
        use mw_core::model::ThemeOverrides;
        let mut d = sample_doc();
        d.meta.theme_overrides = None;
        assert!(
            !file_json(&d).contains("themeOverrides"),
            "没设置就不该多出一个键"
        );
        d.meta.theme_overrides = Some(ThemeOverrides {
            canvas_bg: Some("#fad8df".into()),
            ..Default::default()
        });
        let json = file_json(&d);
        assert!(json.contains("themeOverrides"), "{json}");
        assert!(json.contains("#fad8df"));
        assert!(json.contains("canvasBg"));
    }

    #[test]
    fn the_focus_question_is_written_only_when_set() {
        let mut d = sample_doc();
        assert!(
            !file_json(&d).contains("focusQuestion"),
            "没写聚焦问题就不该多出这个键"
        );
        d.meta.layout.focus_question = Some("为什么要换掉这套 CMS？".into());
        let json = file_json(&d);
        assert!(json.contains("focusQuestion"), "{json}");
        assert!(json.contains("为什么要换掉这套 CMS？"), "{json}");
    }

    #[test]
    fn an_older_file_without_the_focus_question_still_opens() {
        // 老文件的 layout 里没有这个键。走的就是这条反序列化路径：
        // 容器上的 #[serde(default)] 会让缺的字段取 Default，也就是 None。
        let legacy: mw_core::model::LayoutSettings =
            serde_json::from_str(r#"{"kind":"conceptMap","direction":"auto"}"#).unwrap();
        assert_eq!(legacy.focus_question, None, "老文件应读成「没写」");
        assert_eq!(legacy.kind, "conceptMap", "其它字段照常");
        // 反向：写了就要能读回来，否则等于存了个没人认的键。
        let round: mw_core::model::LayoutSettings =
            serde_json::from_str(r#"{"kind":"conceptMap","focusQuestion":"这张图要回答什么？"}"#)
                .unwrap();
        assert_eq!(round.focus_question.as_deref(), Some("这张图要回答什么？"));
    }

    #[test]
    fn the_arrow_head_is_written_only_when_it_is_not_the_default() {
        let mut d = sample_doc();
        let root = d.root_id.clone();
        let to = d
            .doc_order()
            .into_iter()
            .find(|id| id != &root)
            .expect("子节点");
        d.apply(&mw_core::command::Command::AddRelationship {
            from: root.clone(),
            to,
            label: Some("支撑".into()),
        })
        .expect("加关系线");
        let json = file_json(&d);
        assert!(
            !json.contains("\"head\""),
            "默认的前向箭头不该落盘（否则每个文件的 diff 都是脏的）：{json}"
        );
        let rid = d.nodes[&root].relationships[0].id.clone();
        d.apply(&mw_core::command::Command::SetRelationshipHead {
            from: root.clone(),
            id: rid,
            head: mw_core::model::ArrowHead::Both,
        })
        .expect("改方向");
        let json = file_json(&d);
        assert!(json.contains("\"head\":\"both\""), "{json}");
    }

    #[test]
    fn an_older_relationship_without_the_arrow_head_still_opens() {
        // 老文件的关系线没有 head 键：读出来必须是 Forward（= 今天画的样子），
        // 否则老图会突然变成没有箭头的连接线。
        let old: mw_core::model::Relationship =
            serde_json::from_str(r#"{"id":"r1","to":"n2"}"#).unwrap();
        assert_eq!(old.head, mw_core::model::ArrowHead::Forward);
        let round: mw_core::model::Relationship =
            serde_json::from_str(r#"{"id":"r1","to":"n2","head":"off"}"#).unwrap();
        assert_eq!(
            round.head,
            mw_core::model::ArrowHead::Off,
            "写了就要读得回来"
        );
    }
    #[test]
    fn an_older_file_without_the_accent_key_still_opens() {
        let mut content: serde_json::Value =
            serde_json::from_str(&file_json(&sample_doc())).unwrap();
        assert!(
            content
                .as_object_mut()
                .unwrap()
                .remove("themeAccent")
                .is_some(),
            "示例文档本来就该带这个键，删掉才像老文件"
        );
        let manifest = r#"{"formatVersion":1,"app":"x","docId":"d","title":"t","createdAt":0,"modifiedAt":0,"nodeCount":5}"#;
        let bytes = build_zip(manifest, &serde_json::to_string(&content).unwrap());
        let loaded = load_from_bytes(&bytes).unwrap();
        assert_eq!(loaded.doc.meta.theme_accent, None);
        assert_eq!(loaded.doc.meta.theme, "mint");
        assert_eq!(loaded.doc.node_count(), sample_doc().node_count());
    }

    fn build_zip(manifest: &str, content: &str) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            zip.start_file("mimetype", opts).unwrap();
            zip.write_all(MIME.as_bytes()).unwrap();
            zip.start_file("manifest.json", opts).unwrap();
            zip.write_all(manifest.as_bytes()).unwrap();
            zip.start_file("content.json", opts).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn rejects_newer_format_version() {
        let manifest = r#"{"formatVersion":99,"app":"x","docId":"d","title":"t","createdAt":0,"modifiedAt":0,"nodeCount":1}"#;
        let bytes = build_zip(manifest, "{}");
        match load_from_bytes(&bytes) {
            Err(FormatError::TooNew(v)) => assert_eq!(v, 99),
            Err(e) => panic!("应拒绝更高版本，实际错误：{e:?}"),
            Ok(_) => panic!("应拒绝更高版本，但解析成功了"),
        }
    }

    #[test]
    fn rejects_broken_content_json() {
        let manifest = r#"{"formatVersion":1,"app":"x","docId":"d","title":"t","createdAt":0,"modifiedAt":0,"nodeCount":1}"#;
        let bytes = build_zip(manifest, "{ not json");
        assert!(matches!(load_from_bytes(&bytes), Err(FormatError::Json(_))));
    }

    #[test]
    fn rejects_missing_content_entry() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            zip.start_file("manifest.json", opts).unwrap();
            zip.write_all(b"{}").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(
            load_from_bytes(&buf.into_inner()),
            Err(FormatError::Invalid(_))
        ));
    }

    // ---------------------------------------------------------------- SR-01 保存可靠性

    /// 一份「新版本」文档：所有后来才加的可选字段都设上了，当剥键测试的底子。
    ///
    /// 刻意**不**给节点挂卡片：`Node.cardId` 指向的卡片必须存在，否则 `validate()` 判脏，
    /// 「老文件没有 cards 键」那条用例就没法剥键了。
    fn modern_doc() -> Document {
        let mut d = sample_doc();
        d.meta.theme_overrides = Some(mw_core::model::ThemeOverrides {
            canvas_bg: Some("#fad8df".into()),
            ..Default::default()
        });
        d.meta.layout.focus_question = Some("为什么要换掉这套 CMS？".into());
        d.cards
            .push(NoteCard::new(d.id.clone(), None, "独立卡", "# 正文", 5));
        let owner = d
            .nodes
            .iter()
            .find(|(_, n)| !n.relationships.is_empty())
            .map(|(id, _)| id.clone())
            .expect("示例文档里有一条关系线");
        d.nodes.get_mut(&owner).unwrap().relationships[0].head = mw_core::model::ArrowHead::Both;
        d.validate().unwrap();
        d
    }

    /// 手搓容器时要用的 manifest：`docId` 必须与文档 id 一致，否则保存时未知键对不上账。
    fn manifest_of(doc: &Document) -> String {
        serde_json::json!({
            "formatVersion": FORMAT_VERSION,
            "app": APP_ID,
            "docId": doc.id,
            "title": doc.meta.title,
            "createdAt": 1_700_000_000_000i64,
            "modifiedAt": 1_700_000_000_000i64,
            "nodeCount": doc.node_count(),
        })
        .to_string()
    }

    /// 老文件：`themeAccent` / `themeOverrides` / `layout.focusQuestion` /
    /// 所有 `relationships[].head` / `cards` 一个都没有，仍然要能开、能校验。
    #[test]
    fn a_legacy_file_without_any_newer_optional_keys_still_opens() {
        let doc = modern_doc();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        {
            let obj = content.as_object_mut().unwrap();
            for key in ["themeAccent", "themeOverrides", "cards"] {
                obj.remove(key);
            }
            obj["layout"]
                .as_object_mut()
                .unwrap()
                .remove("focusQuestion");
            for node in obj["nodes"].as_object_mut().unwrap().values_mut() {
                let Some(rels) = node.get_mut("relationships").and_then(|v| v.as_array_mut())
                else {
                    continue;
                };
                for rel in rels.iter_mut() {
                    rel.as_object_mut().unwrap().remove("head");
                }
            }
        }
        // 剥不干净的话这条用例就是在自欺欺人。
        let text = serde_json::to_string(&content).unwrap();
        for key in [
            "themeAccent",
            "themeOverrides",
            "focusQuestion",
            "\"cards\"",
            "\"head\"",
        ] {
            assert!(!text.contains(key), "剥键没剥干净：{key} 还在");
        }

        let bytes = build_zip(&manifest_of(&doc), &text);
        let loaded = load_from_bytes(&bytes).unwrap();
        let back = &loaded.doc;
        assert_eq!(back.meta.theme_accent, None, "缺键 = 没设置强调色");
        assert_eq!(back.meta.theme_overrides, None);
        assert_eq!(back.meta.layout.focus_question, None);
        assert!(back.cards.is_empty(), "老文件没有卡片");
        assert!(back.validate().is_ok(), "老文件的结构必须合法");
        assert_eq!(back.node_count(), doc.node_count());
        let owner = back
            .nodes
            .values()
            .find(|n| !n.relationships.is_empty())
            .unwrap();
        assert_eq!(
            owner.relationships[0].head,
            mw_core::model::ArrowHead::Forward,
            "缺 head 的老关系线读出来必须是 Forward（= 今天画的样子）"
        );
        assert!(
            cached_extras(&doc.id)
                .expect("加载过就该有账本条目")
                .is_empty(),
            "老文件一个未知键都没有，不该多捡东西"
        );
    }

    /// 未来未知键：doc 级与 node 级各一个，仍要能开、能校验，而且要被原样捡起来。
    #[test]
    fn a_file_with_future_unknown_keys_still_opens() {
        let doc = modern_doc();
        let root = doc.root_id.clone();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        content["futureKey"] = serde_json::json!(1);
        content["nodes"][root.as_str()]["futureKey"] = serde_json::json!(2);

        let bytes = build_zip(&manifest_of(&doc), &content.to_string());
        let loaded = load_from_bytes(&bytes).unwrap();
        assert_eq!(
            loaded.doc.meta.theme, doc.meta.theme,
            "未知键不该动已知字段"
        );
        assert_eq!(loaded.doc.node_count(), doc.node_count());
        assert!(loaded.doc.validate().is_ok(), "未知键不该影响结构合法性");

        let extras = cached_extras(&doc.id).expect("加载过就该有账本条目");
        assert_eq!(
            extras.doc.get("futureKey"),
            Some(&serde_json::json!(1)),
            "doc 级未知键要捡起来"
        );
        assert_eq!(
            extras.nodes[&root].get("futureKey"),
            Some(&serde_json::json!(2)),
            "node 级未知键要捡起来"
        );
    }

    /// 保真：读出来再存回去，doc 级与 node 级的未知键都还在。
    ///
    /// 这是「新版本写了字段 → 用户用旧版本打开、改一个字、保存」那条路径：
    /// 少了这一步，用户每用旧版本存一次就静默丢一批新字段。
    #[test]
    fn unknown_keys_survive_a_read_save_read_roundtrip() {
        let doc = modern_doc();
        let root = doc.root_id.clone();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        content["futureKey"] = serde_json::json!(1);
        content["nodes"][root.as_str()]["futureNote"] = serde_json::json!("未来版本写的");
        // 真的落到盘上再 `load`：账本按「(docId, 源路径)」记账，从字节读来的没有源路径、
        // 保存时会保守地不写回（见 `extra_keys_are_not_written_when_the_source_path_differs`）。
        let path = temp_path("保真.zmind");
        std::fs::write(&path, build_zip(&manifest_of(&doc), &content.to_string())).unwrap();
        let loaded = load(&path).unwrap();

        save(&loaded.doc, &path).unwrap();
        let saved = content_value_of(&path);
        assert_eq!(saved["futureKey"], serde_json::json!(1), "doc 级未知键丢了");
        assert_eq!(
            saved["nodes"][root.as_str()]["futureNote"],
            serde_json::json!("未来版本写的"),
            "节点级未知键丢了"
        );

        let again = load(&path).unwrap();
        assert!(again.doc.validate().is_ok(), "再读一遍仍要合法");
        assert_eq!(again.doc.node_count(), loaded.doc.node_count());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 未知键只跟着「它原来那份文件」走：路径对不上就不写。
    ///
    /// 两种真实情形：① `另存为`——不能把原来文件的未知键塞进新文件；
    /// ② 同一份文档的两份副本（`docId` 相同）同时打开——A 的未知键绝不能写到 B 上。
    /// 这里对 ① 与「从字节读来再保存」都钉一遍：宁可丢未知键，也不要写错文件。
    #[test]
    fn extra_keys_are_not_written_when_the_source_path_differs() {
        let doc = modern_doc();
        let root = doc.root_id.clone();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        content["futureKey"] = serde_json::json!(1);
        content["nodes"][root.as_str()]["futureNote"] = serde_json::json!("未来版本写的");
        let bytes = build_zip(&manifest_of(&doc), &content.to_string());

        // ① 另存为：从 A 读、往 B 存。
        let path_a = temp_path("源.zmind");
        std::fs::write(&path_a, &bytes).unwrap();
        let loaded = load(&path_a).unwrap();
        let path_b = path_a.with_file_name("另存为.zmind");
        save(&loaded.doc, &path_b).unwrap();
        let saved = content_value_of(&path_b);
        assert!(
            saved.get("futureKey").is_none(),
            "另存为不该把源文件的未知键塞进新文件"
        );
        assert!(
            saved["nodes"][root.as_str()].get("futureNote").is_none(),
            "另存为不该把源文件的节点级未知键塞进新文件"
        );
        // 但存回源文件时照样保真（上一条用例盯着），这里只确认「路径敏感」这一条。
        save(&loaded.doc, &path_a).unwrap();
        assert_eq!(
            content_value_of(&path_a)["futureKey"],
            serde_json::json!(1),
            "存回源文件时未知键要保真"
        );

        // ② 从字节读来（没有源路径）再保存：同样不写。
        let path_c = temp_path("字节来源.zmind");
        let from_bytes = load_from_bytes(&bytes).unwrap();
        save(&from_bytes.doc, &path_c).unwrap();
        assert!(
            content_value_of(&path_c).get("futureKey").is_none(),
            "不知道从哪读来的，就不该往下写未知键"
        );

        let _ = std::fs::remove_dir_all(path_a.parent().unwrap());
    }

    /// 删掉的节点不能把未知键复活：账本还留着它，但合并只按现存节点 id 走。
    #[test]
    fn a_deleted_node_does_not_resurrect_its_unknown_keys() {
        let doc = modern_doc();
        let victim = doc
            .nodes
            .values()
            .find(|n| !n.relationships.is_empty())
            .expect("示例文档里有一条关系线的节点")
            .id
            .clone();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        content["nodes"][victim.as_str()]["futureNote"] = serde_json::json!("这个节点会被删掉");
        let path = temp_path("删节点.zmind");
        std::fs::write(&path, build_zip(&manifest_of(&doc), &content.to_string())).unwrap();
        let mut loaded = load(&path).unwrap();

        loaded
            .doc
            .apply(&mw_core::Command::DeleteNodes {
                ids: vec![victim.clone()],
            })
            .unwrap();
        save(&loaded.doc, &path).unwrap();

        let saved = content_value_of(&path);
        assert!(
            saved["nodes"].get(victim.as_str()).is_none(),
            "节点已经删了，文件里不该还有它"
        );
        assert!(
            !content_json_of(&path).contains("这个节点会被删掉"),
            "删掉的节点的未知键不许复活"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 未知枚举取值不该让整份文档打不开：`head` 退成 `forward`，`task` 退成「没设置」。
    #[test]
    fn unknown_enum_values_degrade_instead_of_failing_the_load() {
        let doc = modern_doc();
        let root = doc.root_id.clone();
        let owner = doc
            .nodes
            .iter()
            .find(|(_, n)| !n.relationships.is_empty())
            .map(|(id, _)| id.clone())
            .unwrap();
        let mut content: serde_json::Value = serde_json::from_str(&file_json(&doc)).unwrap();
        content["nodes"][owner.as_str()]["relationships"][0]["head"] =
            serde_json::json!("sideways");
        content["nodes"][root.as_str()]["task"] = serde_json::json!("blocked");
        // 彻底未知的**键**同样不该让打开失败（它走的是未知键那条路，不是枚举那条）。
        content["nodes"][root.as_str()]["state"] = serde_json::json!("blocked");

        // 落到盘上再 `load`：未知键账本按路径记账，后面那次保存得对得上源路径。
        let path = temp_path("未知枚举.zmind");
        std::fs::write(&path, build_zip(&manifest_of(&doc), &content.to_string())).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.doc.validate().is_ok());
        assert_eq!(
            loaded.doc.nodes[&owner].relationships[0].head,
            mw_core::model::ArrowHead::Forward,
            "未知箭头取值要退回 Forward"
        );
        assert_eq!(
            loaded.doc.nodes[&root].task, None,
            "未知任务状态按「没设置」处理，不替用户编一个状态"
        );

        save(&loaded.doc, &path).unwrap();
        let saved = content_value_of(&path);
        assert!(
            saved["nodes"][owner.as_str()]["relationships"][0]
                .get("head")
                .is_none(),
            "默认的前向箭头不该落盘"
        );
        assert_eq!(
            saved["nodes"][root.as_str()]["state"],
            serde_json::json!("blocked"),
            "未知键要保真（与未知枚举值分开处理）"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 普通文档（没有任何未知键）的 `content.json` 必须与结构体直接序列化逐字节相同：
    /// 加了 `flatten` 也不许改变既有文件的字节形状。
    #[test]
    fn a_plain_document_writes_the_same_bytes_as_before() {
        let doc = sample_doc();
        let path = temp_path("字节形状.zmind");
        save(&doc, &path).unwrap();
        assert_eq!(content_json_of(&path), file_json(&doc), "键序/字节形状变了");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// tmp 名字不许固定：同目录、同文件名，两次保存的 tmp 必须不同名（双写者不互撞）。
    #[test]
    fn temp_file_names_do_not_collide() {
        let dir = Path::new("D:/somewhere");
        let a = tmp_path(dir, "doc.zmind");
        let b = tmp_path(dir, "doc.zmind");
        assert_ne!(a, b, "两次保存的 tmp 撞名 = 双写者互撞");
        let name = a.file_name().unwrap().to_str().unwrap().to_string();
        assert!(name.starts_with(".doc.zmind."), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
        assert!(
            name.contains(&std::process::id().to_string()),
            "要带 pid：{name}"
        );
    }

    /// 保存前清掉陈旧 tmp（上次崩溃留下的），但**不许动别的写者刚建的**。
    #[test]
    fn saving_sweeps_only_stale_temp_files() {
        let dir = scratch("mw-tmp-sweep");
        let path = dir.join("doc.zmind");
        save(&sample_doc(), &path).unwrap();
        let stale = dir.join(".doc.zmind.1.0.0.tmp");
        let fresh = dir.join(".doc.zmind.2.1.1.tmp");
        std::fs::write(&stale, b"old").unwrap();
        std::fs::write(&fresh, b"live").unwrap();
        // 把陈旧那个的 mtime 拨回两小时前（std 自带 `set_times`，不引依赖）。
        let old = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
        let handle = std::fs::File::options().write(true).open(&stale).unwrap();
        handle
            .set_times(std::fs::FileTimes::new().set_modified(old))
            .unwrap();
        drop(handle);

        save(&sample_doc(), &path).unwrap();
        assert!(!stale.exists(), "一小时前的 tmp 必须被清掉");
        assert!(fresh.exists(), "刚建的 tmp 可能是别的写者正在写的，不能动");
        let ours: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                n.starts_with(".doc.zmind.") && n.ends_with(".tmp") && n != ".doc.zmind.2.1.1.tmp"
            })
            .collect();
        assert!(ours.is_empty(), "保存完不该留下自己的 tmp：{ours:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `.bak` 转存失败：不许吞、不许把正本弄丢——正本还在原地、内容一字不差。
    #[test]
    fn a_failed_backup_move_keeps_the_original_in_place() {
        let doc = sample_doc();
        let path = temp_path("doc.zmind");
        save(&doc, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        // 让 `.bak` 的位置被一个目录占住：把正本 rename 过去必然失败。
        let bak = path.with_file_name("doc.zmind.bak");
        std::fs::create_dir(&bak).unwrap();

        let err = save(&doc, &path);
        assert!(err.is_err(), "转存 .bak 失败必须报错，不能假装存好了");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "正本必须一字不差地留在原地"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 正本 rename（tmp → 正本）失败：必须把 `.bak` 改回原名。
    /// 否则用户看到的是「文件不见了」——比保存失败严重得多。
    #[test]
    fn a_failed_commit_restores_the_original_from_the_backup() {
        let doc = sample_doc();
        let path = temp_path("回滚.zmind");
        save(&doc, &path).unwrap();
        let before = std::fs::read(&path).unwrap();

        FAIL_COMMIT.with(|flag| flag.set(true));
        let err = save(&doc, &path);
        FAIL_COMMIT.with(|flag| flag.set(false));

        assert!(err.is_err(), "改名失败必须报错");
        assert!(path.exists(), "正本必须被 .bak 改回来");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "改回来的必须是原来那份"
        );
        let dir = path.parent().unwrap();
        let leftovers: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(".回滚.zmind.") && n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "回滚后残留 tmp：{leftovers:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
