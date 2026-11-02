//! 链接短语建议（CmapTools 的 linking-phrase 下拉，`docs/30` §5.4.2）。
//!
//! 概念图的边是命题，短语是命题的谓词。同一张图里「导致」「支撑」「包含」会被反复用到，
//! 每次重打一遍既慢又会写出同义不同形的词（「导致」/「造成」/「引起」）。
//! CmapTools 的做法是：输入时把**这张图里已经用过的短语**列出来给用户挑。
//!
//! 本模块只做「从文档里取候选」这一件纯事：不认 UI、不写文档、不做词典、不跨文档。
//! 确定性来自两条：候选计数走 `BTreeMap`，排序以 `(频次降, 长度升, 字典序)` 三键收尾。

use std::collections::BTreeMap;

use crate::document::Document;

/// 下拉最多给几条（`docs/30` §5.4.2）。再多就把画布盖住了。
pub const PHRASE_SUGGEST_LIMIT: usize = 6;

/// 本图已用过的链接短语，按 `prefix` 前缀过滤（大小写不敏感），`exclude` 排除当前这条边的原文。
///
/// - `prefix` 为空（或全是空白）时返回空表：**一个字都没输就不弹**，否则双击短语时
///   会先弹一个跟当前输入无关的列表。
/// - 只做前缀匹配，不做子串/模糊：前缀是可预期的，用户按 `Tab` 采用时不会突然换成别的词。
/// - 频次高的排前面（同一短语在几条边上出现过，就说明它是这张图的常用谓词）。
pub fn linking_phrases(doc: &Document, prefix: &str, exclude: Option<&str>) -> Vec<String> {
    let needle = prefix.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let exclude = exclude.map(|s| s.trim());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for node in doc.nodes.values() {
        for rel in &node.relationships {
            let Some(label) = rel.label.as_deref() else {
                continue;
            };
            let text = label.trim();
            if text.is_empty() || Some(text) == exclude {
                continue;
            }
            if !text.to_lowercase().starts_with(&needle) {
                continue;
            }
            *counts.entry(text.to_string()).or_insert(0) += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.chars().count().cmp(&b.0.chars().count()))
            .then_with(|| a.0.cmp(&b.0))
    });
    rows.into_iter()
        .take(PHRASE_SUGGEST_LIMIT)
        .map(|(text, _)| text)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::model::Node;

    fn doc_with_phrases(labels: &[&str]) -> Document {
        // 每条短语一根独立的概念对，避免平行边规则干扰（概念图允许平行边，但没必要）。
        let mut doc = Document::new("短语");
        let _ = doc.apply(&Command::SetLayout {
            layout: crate::model::LayoutSettings {
                kind: "conceptMap".into(),
                ..Default::default()
            },
        });
        let root = doc.root_id.clone();
        for (i, label) in labels.iter().enumerate() {
            let to = Node::new(crate::model::new_id(), None, format!("概念{i}"));
            let to_id = to.id.clone();
            doc.nodes.insert(to_id.clone(), to);
            doc.nodes
                .get_mut(&root)
                .unwrap()
                .relationships
                .push(crate::model::Relationship {
                    id: crate::model::new_id(),
                    to: to_id,
                    label: Some((*label).to_string()),
                    color: None,
                    head: Default::default(),
                    ..Default::default()
                });
        }
        doc
    }

    #[test]
    fn empty_prefix_offers_nothing() {
        let doc = doc_with_phrases(&["导致"]);
        assert!(linking_phrases(&doc, "", None).is_empty());
        assert!(linking_phrases(&doc, "   ", None).is_empty());
    }

    #[test]
    fn candidates_are_deduped_filtered_and_ordered_by_frequency() {
        let doc = doc_with_phrases(&["导致", "导致", "导致结果", "支撑"]);
        // 「导致」出现两次 → 排在「导致结果」前面（后者只是更长）。
        assert_eq!(
            linking_phrases(&doc, "导致", None),
            vec!["导致".to_string(), "导致结果".to_string()]
        );
        // 前缀不匹配的一个都不给。
        assert!(linking_phrases(&doc, "影响", None).is_empty());
    }

    #[test]
    fn the_current_phrase_is_excluded_but_its_siblings_survive() {
        let doc = doc_with_phrases(&["导致", "导致", "导致结果"]);
        assert_eq!(
            linking_phrases(&doc, "导致", Some("导致")),
            vec!["导致结果".to_string()]
        );
    }

    #[test]
    fn the_list_is_capped_and_deterministic() {
        let labels: Vec<String> = (0..10).map(|i| format!("谓词{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let doc = doc_with_phrases(&refs);
        let got = linking_phrases(&doc, "谓词", None);
        assert_eq!(got.len(), PHRASE_SUGGEST_LIMIT);
        // 频次相同 → 先按长度（全等），再按字典序：谓词0..谓词5（UTF-8 字节序与数字序一致）。
        assert_eq!(got[0], "谓词0");
        assert_eq!(got[5], "谓词5");
    }
}
