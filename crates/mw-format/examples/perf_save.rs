//! P10 measurement helper: time load + repeated saves of a `.zmind`.
//!
//! `cargo run -p mw-format --release --example perf_save -- target/perf-10000.zmind 7`

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: perf_save <doc.zmind> [runs]"));
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(7);

    let t = Instant::now();
    let loaded = mw_format::load(&path).expect("load");
    println!(
        "load\t{:.2}\tms\tnodes={}",
        t.elapsed().as_secs_f64() * 1000.0,
        loaded.doc.node_count()
    );

    let tmp = std::env::temp_dir().join(format!("mw-perf-save-{}.zmind", std::process::id()));
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        mw_format::save(&loaded.doc, &tmp).expect("save");
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let bak = PathBuf::from(format!("{}.bak", tmp.display()));
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(&bak);

    samples.sort_by(|a, b| a.total_cmp(b));
    println!(
        "save\truns={}\tp50={:.2}\tp95={:.2}\tmin={:.2}\tmax={:.2}\tms",
        runs,
        pct(&samples, 0.50),
        pct(&samples, 0.95),
        samples.first().copied().unwrap_or(0.0),
        samples.last().copied().unwrap_or(0.0)
    );

    // P10 后台写法中 UI 线程唯一的工作：克隆一份只含格式层字段的快照。
    let snap_runs = runs.max(5);
    let mut snaps = Vec::with_capacity(snap_runs);
    for _ in 0..snap_runs {
        let t = Instant::now();
        let nodes = loaded.doc.nodes.clone();
        let meta = loaded.doc.meta.clone();
        let id = loaded.doc.id.clone();
        let root = loaded.doc.root_id.clone();
        snaps.push(t.elapsed().as_secs_f64() * 1000.0);
        // 别让优化器把克隆整个消掉。
        std::hint::black_box((nodes.len(), meta, id, root));
    }
    snaps.sort_by(|a, b| a.total_cmp(b));
    println!(
        "snapshot\truns={}\tp50={:.2}\tp95={:.2}\tmin={:.2}\tmax={:.2}\tms\t(UI 线程)",
        snap_runs,
        pct(&snaps, 0.50),
        pct(&snaps, 0.95),
        snaps.first().copied().unwrap_or(0.0),
        snaps.last().copied().unwrap_or(0.0)
    );
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
