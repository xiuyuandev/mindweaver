//! 运行时帧率探针（docs/08 §8）：`MW_STATS=1` 时在右上角画一张小卡。
//!
//! 发布版默认关闭，只有显式设置环境变量才出现——它不是用户功能，是验收工具。
//! `MW_STATS_LOG=<path>` 另外每 60 帧写一行汇总，探针脚本读最后一行断言 P5。
//!
//! 帧间隔只在**连续重绘**时采样（≤250ms），否则空闲时按需重绘的几百毫秒间隔
//! 会把 FPS 拉成个位数。和 `FrameBudget` 用同一个门槛，口径一致。

use std::collections::VecDeque;
use std::io::Write;
use std::time::Instant;

use eframe::egui::{self, Pos2};

use crate::canvas::PerfSnapshot;
use crate::chrome::Type;

/// 与 `mw_render::budget` 的连续重绘门槛一致：超过它视为用户停手/窗口被挂起。
const CONTINUOUS_MAX_MS: f32 = 250.0;
/// 滚动窗口长度：60fps 下约 4 秒。
const WINDOW: usize = 240;
/// 每多少帧写一行日志。
const LOG_EVERY: u32 = 60;

#[derive(Clone, Copy, Debug, Default)]
pub struct Summary {
    pub fps: f32,
    pub p50: f32,
    pub p95: f32,
    pub ui_p50: f32,
    pub ui_p95: f32,
}

pub struct Stats {
    enabled: bool,
    log: Option<std::fs::File>,
    last: Option<Instant>,
    wall: VecDeque<f32>,
    ui: VecDeque<f32>,
    shown: Summary,
    frames: u32,
    scratch: Vec<f32>,
}

impl Stats {
    pub fn from_env() -> Self {
        let enabled = std::env::var_os("MW_STATS").is_some_and(|v| v != "0");
        let log = if enabled {
            std::env::var_os("MW_STATS_LOG").and_then(|path| std::fs::File::create(path).ok())
        } else {
            None
        };
        let mut stats = Self {
            enabled,
            log,
            last: None,
            wall: VecDeque::with_capacity(WINDOW),
            ui: VecDeque::with_capacity(WINDOW),
            shown: Summary::default(),
            frames: 0,
            scratch: Vec::with_capacity(WINDOW),
        };
        if let Some(file) = stats.log.as_mut() {
            let _ = writeln!(
                file,
                "# frame\tfps\twall_p50\twall_p95\tui_p50\tui_p95\tvisible\ttotal\tzoom\tlow_render\tmotion"
            );
            let _ = file.flush();
        }
        stats
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 每帧末尾调用。`ui_ms` 是本帧从进入 `Shell::ui` 到现在的耗时。
    pub fn tick(&mut self, now: Instant, ui_ms: f32, snap: PerfSnapshot) {
        if !self.enabled {
            return;
        }
        if let Some(last) = self.last {
            let dt_ms = now.duration_since(last).as_secs_f32() * 1000.0;
            if dt_ms <= CONTINUOUS_MAX_MS {
                push(&mut self.wall, dt_ms);
                push(&mut self.ui, ui_ms);
            } else {
                // 停手后第一帧的间隔不代表渲染能力，整窗丢掉重新起算。
                self.wall.clear();
                self.ui.clear();
            }
        }
        self.last = Some(now);
        self.frames += 1;

        if self.wall.len() >= 2 {
            let wall: Vec<f32> = self.wall.iter().copied().collect();
            let ui: Vec<f32> = self.ui.iter().copied().collect();
            let mean = wall.iter().sum::<f32>() / wall.len() as f32;
            self.shown = Summary {
                fps: if mean > 0.0 { 1000.0 / mean } else { 0.0 },
                p50: percentile(&mut self.scratch, &wall, 0.50),
                p95: percentile(&mut self.scratch, &wall, 0.95),
                ui_p50: percentile(&mut self.scratch, &ui, 0.50),
                ui_p95: percentile(&mut self.scratch, &ui, 0.95),
            };
        }

        if self.frames.is_multiple_of(LOG_EVERY) && self.wall.len() >= 30 {
            if let Some(file) = self.log.as_mut() {
                let s = self.shown;
                let _ = writeln!(
                    file,
                    "{}\t{:.1}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{:.3}\t{}\t{}",
                    self.frames,
                    s.fps,
                    s.p50,
                    s.p95,
                    s.ui_p50,
                    s.ui_p95,
                    snap.visible,
                    snap.nodes,
                    snap.zoom,
                    snap.low_render as u8,
                    snap.motion as u8,
                );
                let _ = file.flush();
            }
        }
    }

    pub fn show(&self, ctx: &egui::Context, snap: PerfSnapshot) {
        if !self.enabled {
            return;
        }
        let screen = ctx
            .input(|i| i.raw.screen_rect)
            .unwrap_or(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            ));
        let s = self.shown;
        egui::Area::new(egui::Id::unique("mw-perf-stats"))
            .fixed_pos(Pos2::new(screen.max.x - 314.0, 70.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(crate::chrome::c32a(mw_core::design::shell::stats::SCRIM))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_min_width(286.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let line = |ui: &mut egui::Ui, text: String| {
                            ui.label(
                                egui::RichText::new(text)
                                    .size(Type::SMALL)
                                    .color(crate::chrome::c32(mw_core::design::shell::stats::INK)),
                            );
                        };
                        line(
                            ui,
                            format!("FPS {:.1}   帧 {:.1}ms   p95 {:.1}ms", s.fps, s.p50, s.p95),
                        );
                        line(ui, format!("UI {:.1}ms   p95 {:.1}ms", s.ui_p50, s.ui_p95));
                        let share = if snap.nodes > 0 {
                            snap.visible as f32 * 100.0 / snap.nodes as f32
                        } else {
                            0.0
                        };
                        // 与右下角缩放条同一个读数函数（X21）：两处各写一遍格式，
                        // 迟早出现「浮层写 0.5%、缩放条写 0%」。
                        let zoom = crate::canvas::format_zoom(snap.zoom);
                        line(
                            ui,
                            format!(
                                "{} 节点 · 可见 {} ({:.0}%) · {} · {}{}",
                                snap.nodes,
                                snap.visible,
                                share,
                                zoom,
                                if snap.motion { "过渡中" } else { "静止" },
                                if snap.low_render { " · 省渲染" } else { "" }
                            ),
                        );
                    });
            });
    }
}

fn push(ring: &mut VecDeque<f32>, value: f32) {
    if ring.len() == WINDOW {
        ring.pop_front();
    }
    ring.push_back(value);
}

/// 原地排序取分位数；空集返回 0。
fn percentile(scratch: &mut Vec<f32>, values: &[f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    scratch.clear();
    scratch.extend_from_slice(values);
    scratch.sort_by(|a, b| a.total_cmp(b));
    let idx = ((scratch.len() - 1) as f32 * p).round() as usize;
    scratch[idx.min(scratch.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_nearest_rank_and_handles_empty() {
        let mut scratch = Vec::new();
        assert_eq!(percentile(&mut scratch, &[], 0.95), 0.0);
        let v = [4.0, 1.0, 3.0, 2.0];
        assert_eq!(percentile(&mut scratch, &v, 0.50), 3.0);
        assert_eq!(percentile(&mut scratch, &v, 0.95), 4.0);
    }

    #[test]
    fn ring_keeps_the_most_recent_window() {
        let mut ring = VecDeque::new();
        for i in 0..(WINDOW + 10) {
            push(&mut ring, i as f32);
        }
        assert_eq!(ring.len(), WINDOW);
        assert_eq!(ring.front().copied(), Some(10.0));
    }
}
