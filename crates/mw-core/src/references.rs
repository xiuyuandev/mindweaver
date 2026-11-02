//! 卡片引用 `[[目标]]`：目标是卡片 id 或标题。解析只做纯文本扫描，
//! 不改 Markdown 渲染；断链由调用方拿已知卡片列表判断。

/// 扫描正文里的 `[[...]]`，去重、去首尾空白。空目标忽略。
pub fn parse(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let target = after[..end].trim();
        if !target.is_empty()
            && target.chars().count() <= 128
            && !out.iter().any(|t: &String| t == target)
        {
            out.push(target.to_string());
        }
        rest = &after[end + 2..];
    }
    out
}

/// 目标是否指向这张卡片：先比 id，再比标题（忽略大小写）。
pub fn matches(target: &str, id: &str, title: &str) -> bool {
    target == id || target.eq_ignore_ascii_case(title.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_are_deduped_and_broken_markers_ignored() {
        let body = "看 [[a]] 和 [[ b ]]、[[a]]，还有 [[未闭合。";
        assert_eq!(parse(body), vec!["a".to_string(), "b".to_string()]);
        assert!(parse("没有任何引用").is_empty());
    }

    #[test]
    fn target_matches_id_or_title_case_insensitively() {
        assert!(matches("card-1", "card-1", "标题"));
        assert!(matches("标题", "card-1", "标题"));
        assert!(!matches("别的", "card-1", "标题"));
    }
}
