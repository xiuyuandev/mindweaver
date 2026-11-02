//! 主动回忆：本地间隔重复。只存 `key -> 复习状态` 的 JSON，不联网、不上传。
//!
//! 评分只有四档（Again / Hard / Good / Easy），间隔是「天」；`Again` 10 分钟后再来。
//! 算法是简化的 SM-2：难度系数 `ease` 夹在 1.3–3.0，`Good` 按 ease 乘，`Easy` 再乘 1.3。
//! key 由调用方拼：节点 `n:<id>`、卡片 `c:<id>`，两类互不冲突。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const DAY_MS: i64 = 86_400_000;
pub const AGAIN_MS: i64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub ease: f32,
    pub interval: u32,
    pub due: i64,
}

/// 一次评分记录：只存本地天序号，不存时区/网络信息。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// 本地日期序号（1970-01-01 = 0）。
    pub day: i64,
    pub at: i64,
    pub key: String,
    pub grade: Grade,
}

/// 学习统计（全部由本地日志复算）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub total: usize,
    pub today: usize,
    pub streak: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ease: 2.5,
            interval: 0,
            due: 0,
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 今天的本地日期序号。Windows 走 `GetLocalTime`，其它平台退回 UTC 天。
pub fn local_today() -> i64 {
    #[cfg(windows)]
    unsafe {
        let st = windows::Win32::System::SystemInformation::GetLocalTime();
        days_from_civil(st.wYear as i32, st.wMonth as u32, st.wDay as u32)
    }
    #[cfg(not(windows))]
    {
        now_ms() / DAY_MS
    }
}

/// 公历 → 1970-01-01 起的天数（Howard Hinnant 算法），不依赖时区库。
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 连续复习天数：从今天（或昨天，若今天还没复习）往回数。
pub fn streak_days(days: &[i64], today: i64) -> usize {
    let mut set: Vec<i64> = days.to_vec();
    set.sort_unstable();
    set.dedup();
    let mut cur = if set.contains(&today) {
        today
    } else if set.contains(&(today - 1)) {
        today - 1
    } else {
        return 0;
    };
    let mut n = 0;
    while set.contains(&cur) {
        n += 1;
        cur -= 1;
    }
    n
}

/// 评分后的新状态。`now` 显式传入，单测不依赖时钟。
pub fn schedule(cur: State, grade: Grade, now: i64) -> State {
    let mut ease = cur.ease.clamp(1.3, 3.0);
    let interval = match grade {
        Grade::Again => {
            ease = (ease - 0.2).max(1.3);
            0
        }
        Grade::Hard => {
            ease = (ease - 0.15).max(1.3);
            ((cur.interval as f32) * 1.2).round().max(1.0) as u32
        }
        Grade::Good => match cur.interval {
            0 => 1,
            1 => 3,
            n => ((n as f32) * ease).round().max(1.0) as u32,
        },
        Grade::Easy => {
            ease = (ease + 0.15).min(3.0);
            match cur.interval {
                0 => 3,
                n => ((n as f32) * ease * 1.3).round().max(1.0) as u32,
            }
        }
    };
    let due = if matches!(grade, Grade::Again) {
        now + AGAIN_MS
    } else {
        now + interval as i64 * DAY_MS
    };
    State {
        ease,
        interval,
        due,
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    states: BTreeMap<String, State>,
    #[serde(default)]
    log: Vec<Event>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Store {
    pub fn load_default() -> Self {
        match crate::paths::review_file() {
            Some(p) => Self::load_from(p),
            None => Self::default(),
        }
    }

    pub fn load_from(path: PathBuf) -> Self {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // 新格式 `{states, log}`；旧格式是纯 `states` 映射，读进来后日志为空。
        let (states, log) = match serde_json::from_str::<Store>(&text) {
            Ok(store) => (store.states, store.log),
            Err(_) => (
                serde_json::from_str::<BTreeMap<String, State>>(&text).unwrap_or_default(),
                Vec::new(),
            ),
        };
        Self {
            states,
            log,
            path: Some(path),
        }
    }

    pub fn get(&self, key: &str) -> Option<State> {
        self.states.get(key).copied()
    }

    pub fn is_due(&self, key: &str, now: i64) -> bool {
        self.states.get(key).is_none_or(|s| s.due <= now)
    }

    /// 到期排序：逾期最久的在前；没记录的新项视为立即到期（due=0）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn due<'a>(&self, keys: impl Iterator<Item = &'a str>, now: i64) -> Vec<String> {
        let mut out: Vec<(i64, String)> = keys
            .filter(|k| self.is_due(k, now))
            .map(|k| (self.states.get(k).map_or(0, |s| s.due), k.to_string()))
            .collect();
        out.sort();
        out.into_iter().map(|(_, k)| k).collect()
    }

    pub fn grade(&mut self, key: &str, grade: Grade) -> State {
        let now = now_ms();
        let next = schedule(self.get(key).unwrap_or_default(), grade, now);
        self.states.insert(key.to_string(), next);
        self.log.push(Event {
            day: local_today(),
            at: now,
            key: key.to_string(),
            grade,
        });
        // 日志只保最近 10000 条，避免无限增长。
        if self.log.len() > 10_000 {
            let drop = self.log.len() - 10_000;
            self.log.drain(0..drop);
        }
        self.save();
        next
    }

    /// 学习统计：总复习数、今日数、连续天数。`today` 显式传入便于单测。
    pub fn stats(&self, today: i64) -> Stats {
        let days: Vec<i64> = self.log.iter().map(|e| e.day).collect();
        Stats {
            total: self.log.len(),
            today: days.iter().filter(|d| **d == today).count(),
            streak: streak_days(&days, today),
        }
    }

    pub fn save(&self) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self) {
            let _ = std::fs::write(path, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_days_and_streaks_are_recomputable() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        let t = days_from_civil(2026, 10, 20);
        assert_eq!(streak_days(&[t, t - 1, t - 2], t), 3);
        assert_eq!(streak_days(&[t - 1, t - 2], t), 2, "昨天复习过，连续未断");
        assert_eq!(streak_days(&[t - 2], t), 0, "断了");
        assert_eq!(streak_days(&[t, t, t - 1], t), 2, "同一天多次只算一天");
    }

    #[test]
    fn stats_count_today_and_total() {
        let mut s = Store::default();
        let t = 20_000;
        for (i, day) in [t - 2, t - 1, t, t].into_iter().enumerate() {
            s.log.push(Event {
                day,
                at: i as i64,
                key: "n:a".into(),
                grade: Grade::Good,
            });
        }
        let st = s.stats(t);
        assert_eq!(st.total, 4);
        assert_eq!(st.today, 2);
        assert_eq!(st.streak, 3);
    }

    #[test]
    fn good_walks_one_three_then_multiplies_by_ease() {
        let now = 0;
        let a = schedule(State::default(), Grade::Good, now);
        assert_eq!(a.interval, 1);
        assert_eq!(a.due, now + DAY_MS);
        let b = schedule(a, Grade::Good, now);
        assert_eq!(b.interval, 3);
        let c = schedule(b, Grade::Good, now);
        assert_eq!(c.interval, 8, "3 × 2.5 ≈ 8");
    }

    #[test]
    fn again_resets_interval_and_returns_in_ten_minutes() {
        let s = State {
            ease: 2.5,
            interval: 12,
            due: 0,
        };
        let a = schedule(s, Grade::Again, 1_000);
        assert_eq!(a.interval, 0);
        assert_eq!(a.due, 1_000 + AGAIN_MS);
        assert!(a.ease < s.ease);
    }

    #[test]
    fn ease_is_clamped_and_due_orders_the_queue() {
        let mut s = State::default();
        for _ in 0..20 {
            s = schedule(s, Grade::Easy, 0);
        }
        assert!(s.ease <= 3.0);
        let mut store = Store::default();
        store.states.insert(
            "n:a".into(),
            State {
                ease: 2.5,
                interval: 1,
                due: 5,
            },
        );
        store.states.insert(
            "n:b".into(),
            State {
                ease: 2.5,
                interval: 1,
                due: 1,
            },
        );
        let due = store.due(["n:a", "n:b", "n:c"].into_iter(), 10);
        assert_eq!(due, vec!["n:c", "n:b", "n:a"]);
    }
}
