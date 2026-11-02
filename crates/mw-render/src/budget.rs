//! 帧预算监控（docs/08 P5、§6）：连续 3 秒低于 45fps 就进省渲染。
//!
//! 只在**连续重绘**（拖动、缩放、惯性、动画）期间采样：空闲时 egui 按需重绘，
//! 两帧之间可能是几秒，按墙钟间隔算会立刻误判。进入省渲染后本进程保持该档位，
//! 因为降级的诱因（低档机、超大窗口）不会自己消失；等设置面板（M3）再做手动覆盖。

use std::time::{Duration, Instant};

/// 进入省渲染的帧率门槛。
pub const LOW_FPS: f32 = 45.0;
/// 连续低于门槛多久才降级。
pub const LOW_AFTER: Duration = Duration::from_secs(3);
/// 两帧间隔超过这个值就不再算「连续重绘」，视为用户停手或窗口被挂起。
const CONTINUOUS_MAX: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
pub struct FrameBudget {
    last: Option<Instant>,
    slow_run: Duration,
    low: bool,
}

impl FrameBudget {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧调用一次。`active` = 本帧是用户持续操作还是按需重绘。
    pub fn tick(&mut self, active: bool) -> bool {
        self.tick_at(Instant::now(), active)
    }

    pub fn tick_at(&mut self, now: Instant, active: bool) -> bool {
        let gap = self.last.map(|last| now.saturating_duration_since(last));
        self.last = Some(now);

        if !active {
            self.slow_run = Duration::ZERO;
            return self.low;
        }
        let Some(gap) = gap else {
            return self.low;
        };
        if gap > CONTINUOUS_MAX {
            self.slow_run = Duration::ZERO;
            return self.low;
        }

        if gap.as_secs_f32() * LOW_FPS > 1.0 {
            self.slow_run += gap;
            if self.slow_run >= LOW_AFTER {
                self.low = true;
            }
        } else {
            self.slow_run = Duration::ZERO;
        }
        self.low
    }

    pub fn low_render(&self) -> bool {
        self.low
    }

    pub fn reset(&mut self) {
        self.last = None;
        self.slow_run = Duration::ZERO;
        self.low = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(fps: f32) -> Duration {
        Duration::from_secs_f32(1.0 / fps)
    }

    #[test]
    fn three_seconds_below_45_fps_enters_low_render() {
        let mut b = FrameBudget::new();
        let mut t = Instant::now();
        // 30fps：每帧 33.3ms；90 帧 = 2.999s，还不该降级
        for _ in 0..90 {
            t += step(30.0);
            assert!(!b.tick_at(t, true), "3 秒之前不该提前降级");
        }
        for _ in 0..2 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        assert!(b.low_render(), "30fps 持续 3 秒必须进省渲染");
    }

    #[test]
    fn good_frames_reset_the_run() {
        let mut b = FrameBudget::new();
        let mut t = Instant::now();
        for _ in 0..80 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        for _ in 0..10 {
            t += step(60.0);
            b.tick_at(t, true);
        }
        for _ in 0..30 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        assert!(!b.low_render(), "中途恢复 60fps 后，慢帧累计必须清零");
    }

    #[test]
    fn idle_frames_do_not_count() {
        let mut b = FrameBudget::new();
        let mut t = Instant::now();
        // 用户停手：即使间隔很长，也不该触发降级
        for _ in 0..20 {
            t += Duration::from_secs(2);
            b.tick_at(t, false);
        }
        assert!(!b.low_render());
        // 再连续掉帧 3 秒仍然照常触发
        for _ in 0..100 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        assert!(b.low_render());
    }

    #[test]
    fn long_gap_breaks_continuity() {
        let mut b = FrameBudget::new();
        let mut t = Instant::now();
        for _ in 0..80 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        t += Duration::from_millis(400); // 窗口被拖住/挂起
        b.tick_at(t, true);
        for _ in 0..10 {
            t += step(30.0);
            b.tick_at(t, true);
        }
        assert!(!b.low_render(), "中断后的慢帧不该接上中断前的累计");
    }
}
