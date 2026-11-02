use std::path::PathBuf;

use mw_format::{load_from_bytes, save, FORMAT_VERSION};

const TINY: &[u8] = include_bytes!("../../../tests/fixtures/x100/tiny-cn.zmind");
const COMPLEX: &[u8] = include_bytes!("../../../tests/fixtures/x100/complex-style.zmind");

/// 落地目录：仓库内 target/tmp 优先（本机用户 Temp 对测试进程偶发拒写，os error 5）。
fn scratch() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/tests");
    let base = if std::fs::create_dir_all(&base).is_ok() {
        base
    } else {
        std::env::temp_dir()
    };
    base.join(format!("mw-x100-format-{}", std::process::id()))
}

fn temp_path(name: &str) -> PathBuf {
    let dir = scratch();
    std::fs::create_dir_all(&dir).expect("测试临时目录必须能创建");
    dir.join(name)
}

#[test]
fn x100_committed_fixtures_load_and_roundtrip() {
    for (name, bytes, expected_nodes) in [("tiny", TINY, 11usize), ("complex", COMPLEX, 126)] {
        let first = load_from_bytes(bytes).unwrap();
        assert_eq!(first.manifest.format_version, FORMAT_VERSION);
        assert_eq!(first.doc.node_count(), expected_nodes, "{name}");
        assert!(first.doc.validate().is_ok(), "{name} structural validation");

        let path = temp_path(&format!("{name}.zmind"));
        save(&first.doc, &path).unwrap();
        let second = mw_format::load(&path).unwrap();
        assert_eq!(second.doc.root_id, first.doc.root_id);
        assert_eq!(second.doc.node_count(), first.doc.node_count());
        assert_eq!(second.doc.cards.len(), first.doc.cards.len());
        assert_eq!(second.doc.assets.len(), first.doc.assets.len());
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn truncated_fixture_is_rejected_instead_of_partially_loaded() {
    let cut = TINY.len() / 3;
    assert!(load_from_bytes(&TINY[..cut]).is_err());
}
