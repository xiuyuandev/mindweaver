//! 生成一棵 N 节点的合成 `.zmind`，供启动/内存探针使用。
//! `cargo run -p mw-shell --release --example gen_big -- 20000 target/big-20k.zmind`

use std::collections::{HashMap, VecDeque};

use mw_core::{DocMeta, Document, Node};

fn main() {
    let mut args = std::env::args().skip(1);
    let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let out = args.next().unwrap_or_else(|| "target/big-20k.zmind".into());
    const WIDTHS: [usize; 4] = [6, 8, 8, 5];
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let mut root = Node::new("n0".into(), None, "中心主题");
    root.modified_at = 1;
    nodes.insert("n0".into(), root);
    let mut queue = VecDeque::from([("n0".to_string(), 0usize)]);
    let mut next = 1usize;
    while next < count {
        let Some((parent, depth)) = queue.pop_front() else {
            break;
        };
        for _ in 0..WIDTHS[depth % WIDTHS.len()] {
            if next >= count {
                break;
            }
            let id = format!("n{next}");
            let mut n = Node::new(id.clone(), Some(parent.clone()), format!("分支节点 {next}"));
            n.modified_at = 1;
            if next.is_multiple_of(40) {
                n.labels = vec!["重要".into()];
            }
            nodes.get_mut(&parent).unwrap().children.push(id.clone());
            nodes.insert(id.clone(), n);
            queue.push_back((id, depth + 1));
            next += 1;
        }
    }
    let meta = DocMeta {
        title: format!("{count} 节点压力文档"),
        ..Default::default()
    };
    let doc = Document::from_parts(None, meta, "n0".into(), nodes).expect("合法树");
    let path = std::path::Path::new(&out);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    mw_format::save(&doc, path).unwrap();
    println!(
        "wrote {} ({} nodes, {} bytes)",
        path.display(),
        doc.node_count(),
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    );
}
