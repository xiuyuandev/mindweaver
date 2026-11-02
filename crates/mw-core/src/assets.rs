//! 图片/附件字节与内容哈希。资源不进 `content.json`，只进 `.zmind` 的 `assets/`，
//! 同一个哈希只存一份；加载时缺资源也不报错（渲染层降级成占位）。

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

/// 资源键：SHA-256 十六进制。节点用 `asset:<hash>` 引用。
pub fn hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// 文档资源表：hash → 原始字节。保持 BTreeMap，保存时条目顺序稳定。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assets {
    map: BTreeMap<String, Vec<u8>>,
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }

    /// 加入一份资源；已存在同哈希时**不重复存**，返回同一个键。
    pub fn insert(&mut self, bytes: Vec<u8>) -> String {
        let key = hash(&bytes);
        self.map.entry(key.clone()).or_insert(bytes);
        key
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.map.get(key).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.map.iter()
    }

    /// 加载时直接塞入（key 来自 ZIP 条目名）；坏条目由调用方跳过。
    pub fn put(&mut self, key: String, bytes: Vec<u8>) {
        self.map.insert(key, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bytes_store_once() {
        let mut a = Assets::new();
        let k1 = a.insert(vec![1, 2, 3]);
        let k2 = a.insert(vec![1, 2, 3]);
        assert_eq!(k1, k2);
        assert_eq!(a.len(), 1);
        assert_eq!(a.get(&k1), Some(&[1u8, 2, 3][..]));
        assert_eq!(k1.len(), 64, "SHA-256 十六进制 64 字符");
    }

    #[test]
    fn different_bytes_get_different_keys() {
        let mut a = Assets::new();
        assert_ne!(a.insert(vec![1]), a.insert(vec![2]));
        assert_eq!(a.len(), 2);
    }
}
