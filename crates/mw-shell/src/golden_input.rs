//! 金样输入脚本：`MW_GOLDEN_INPUT=<文件>` 时，按帧把键盘 / IME / 指针事件注入
//! `egui::RawInput`。
//!
//! 为什么不走 SendKeys：前台窗口随时可能被别的东西抢走，按键会落到别的应用里；
//! 输入法组合又会截走 Enter。这里在进程内、在 `raw_input_hook` 里注入，验收
//! 结果与桌面焦点、输入法状态都无关，跑一百次也是同一串事件。
//!
//! 一行一个动作，一帧执行一个动作（`key` 占两帧：按下、松开）：
//!
//! ```text
//! wait 3                      # 空跑 3 帧
//! key ctrl+a                  # 修饰键 + 名称（Key::from_name 的写法）
//! text ABCDEF                 # 直接落文字（等价于已提交的输入）
//! ime preedit 你好            # 组合串（未确认）
//! ime commit 你好             # 确认组合
//! click 180 392               # 指针移入并按下，下一帧松开
//! sclick 180 392              # Shift+点击（扩选/缩选）
//! mods ctrl                   # 立起修饰键状态（拖拽的 Ctrl/Shift 在按下那一刻就读走）
//! doubleclick 180 392         # 双击（四帧：按/放/按/放）
//! middleclick 180 392         # 中键单击（标签页中键关闭）
//! press 120 430               # 按住不放（拖拽起点）
//! move 180 520                # 指针移动（拖拽/悬停）
//! release 180 520             # 松开（拖拽落点）
//! hover 180 520 700           # 移到目标并保持 700ms（测试 hover 计时）
//! scroll 0 -240               # 触控板/高精度滚动（Point）；正数向右下
//! scroll_lines 0 -3           # 普通鼠标滚轮（Line，一格 ±1）；egui 会乘速度
//!                             # 滚轮事件自带「当前按下的修饰键」：`mods ctrl` + `scroll` 即 Ctrl+滚轮 = 缩放
//! paste 甲\n  乙            # 系统剪贴板文本（\n / \t 可转义）
//! key_paste ctrl+shift+V 甲\n  乙  # 同帧按键 + 剪贴板文本
//! sleep 700                   # 按真实毫秒等待（自动展开/动效用）
//! ```

use std::time::{Duration, Instant};

use eframe::egui::{
    Event, ImeEvent, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2,
    RawInput, TouchPhase, Vec2,
};

pub struct GoldenInput {
    steps: Vec<Step>,
    next: usize,
    applied: usize,
    /// 脚本里有没有指针动作。有则指针归脚本管，harness 不能每帧清掉它，
    /// 否则 `hover` 的 120/160ms 计时永远攒不满，握把/悬停态就没了。
    pointer: bool,
    /// `sleep` 在跑真实毫秒；到期前不推进脚本，也不让 harness 截图。
    sleep_until: Option<Instant>,
    /// 脚本开始时已经写过 trace；结束时再补一条，证据工具能区分「没跑」和「跑完」。
    done_traced: bool,
}

#[derive(Debug)]
struct Step {
    label: String,
    events: Vec<Event>,
    sleep: Option<Duration>,
}

impl GoldenInput {
    /// `enabled` 只在快照 harness 里为真：交互式启动不看这个环境变量，
    /// 免得误把正常使用变成一个自动化回放。
    pub fn from_env(enabled: bool) -> Option<Self> {
        if !enabled {
            return None;
        }
        let path = std::env::var_os("MW_GOLDEN_INPUT")?;
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("[golden] 读输入脚本失败 {}：{e}", path.display());
            std::process::exit(2);
        });
        let steps = match parse(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[golden] 输入脚本有误 {}：{e}", path.display());
                std::process::exit(2);
            }
        };
        println!("[golden] 输入脚本 {} · {} 帧", path.display(), steps.len());
        let pointer = steps.iter().any(|s| {
            s.events.iter().any(|e| {
                matches!(
                    e,
                    Event::PointerMoved(_)
                        | Event::PointerButton { .. }
                        | Event::PointerGone
                        | Event::MouseWheel { .. }
                )
            })
        });
        crate::trace::event(&format!(
            "golden_input_start steps={} source={}",
            steps.len(),
            path.display()
        ));
        Some(Self {
            steps,
            next: 0,
            applied: 0,
            pointer,
            sleep_until: None,
            done_traced: false,
        })
    }

    /// 脚本是否自己驱动指针。纯键盘/IME 脚本不碰指针，harness 必须每帧清掉
    /// 桌面光标，否则鼠标恰好压在某行上会把 hover 截进基线（同一份构建出两张图）。
    pub fn owns_pointer(&self) -> bool {
        self.pointer
    }

    /// 脚本占用的帧数（含 wait）。harness 的 warmup 至少要覆盖它。
    pub fn frames(&self) -> usize {
        self.steps.len()
    }

    /// 脚本是否已经跑完（`sleep` 到期才算）。
    pub fn finished(&self) -> bool {
        self.next >= self.steps.len() && self.sleep_until.is_none()
    }

    /// 虚拟时钟步进（每帧调一次，早于 [`Self::apply`]）。
    ///
    /// 金样的截帧只按帧数走，真实帧耗时不能改变「第 N 帧长什么样」：
    /// 同一行悬停 120ms 才露握把、折叠行 500ms 自动展开、滚动落定动画，
    /// 门里第一遍机器还在降载时慢帧会让这些状态提前或滞后一帧。
    ///
    /// 普通帧推进 1/60 秒；`sleep` 期间冻结（这段时间本来也不截帧），
    /// 到期那一帧把脚本里写的名义时长一次补上——总时长与帧率无关。
    pub fn advance_clock(&mut self) -> f64 {
        const FRAME: f64 = 1.0 / 60.0;
        let Some(until) = self.sleep_until else {
            return FRAME;
        };
        if Instant::now() < until {
            return 0.0;
        }
        self.sleep_until = None;
        let dur = self
            .steps
            .get(self.next)
            .and_then(|s| s.sleep)
            .map_or(0.0, |d| d.as_secs_f64());
        self.next += 1;
        self.trace_done_once();
        dur
    }

    /// 每帧调一次：把这一行的事件追加进原始输入。
    pub fn apply(&mut self, raw: &mut RawInput) {
        // 睡眠期间脚本不推进，时钟也停在原地（见 `advance_clock`）。
        if self.sleep_until.is_some() {
            return;
        }
        let Some(step) = self.steps.get(self.next) else {
            return;
        };
        if let Some(d) = step.sleep {
            self.sleep_until = Some(Instant::now() + d);
            return;
        }
        self.next += 1;
        self.applied += 1;
        crate::trace::event(&format!("golden_input {} {}", self.applied, step.label));
        for ev in &step.events {
            raw.events.push(ev.clone());
        }
        self.trace_done_once();
    }

    fn trace_done_once(&mut self) {
        if self.done_traced || self.next < self.steps.len() {
            return;
        }
        self.done_traced = true;
        crate::trace::event(&format!(
            "golden_input_done steps={} applied={}",
            self.steps.len(),
            self.applied
        ));
    }
}

/// 解析脚本。`wait N` 展开成 N 个空帧，其余一行一帧。
fn parse(raw: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    // 当前立着的修饰键（只由 `mods` 行改）。滚轮事件要把这一格**抄进事件本身**：
    // egui 判「Ctrl+滚轮 = 缩放」读的是 `Event::MouseWheel.modifiers`
    // （`WheelState::modifiers`），不是 `InputState.modifiers`——只发 `ModifiersChanged`
    // 的话手势会被读成平移，画布缩放的 golden 根本拍不出来。
    let mut held_mods = Modifiers::NONE;
    for (lineno, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (verb, rest) = line
            .split_once(char::is_whitespace)
            .map(|(v, r)| (v, r.trim()))
            .unwrap_or((line, ""));
        let err = |m: &str| format!("第 {} 行：「{line}」{m}", lineno + 1);
        match verb {
            "wait" => {
                let n: usize = rest.parse().map_err(|_| err("的帧数不是数字"))?;
                for _ in 0..n {
                    steps.push(Step {
                        label: "wait".into(),
                        events: Vec::new(),
                        sleep: None,
                    });
                }
            }
            "key" => {
                let (mods, name) = split_key(rest);
                let key = name
                    .and_then(Key::from_name)
                    .ok_or_else(|| err("认不出这个键名"))?;
                steps.push(Step {
                    label: format!("key {}{}", mods_label(mods), key.name()),
                    sleep: None,
                    events: vec![
                        Event::ModifiersChanged(mods),
                        Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: mods,
                        },
                    ],
                });
                // 松开放在下一帧：egui 靠按下/松开维护 keys_down，缺了松开，
                // 后面同键的按下会被标成 repeat。
                steps.push(Step {
                    label: format!("release {}", key.name()),
                    sleep: None,
                    events: vec![
                        Event::Key {
                            key,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: mods,
                        },
                        Event::ModifiersChanged(Modifiers::NONE),
                    ],
                });
            }
            "text" => {
                // `\s` 是空格：脚本每行都被 trim 过，行尾空格留不住，而「敲空格」正是
                // 触发块标记完成（`# ` / `> ` / `- ` / `1. `）的那一格，必须能单独发。
                // `\n` / `\t` 与 `paste` 保持同一套写法；其余反斜杠原样保留。
                let text = unescape(rest);
                steps.push(Step {
                    label: format!("text {text:?}"),
                    events: vec![Event::Text(text)],
                    sleep: None,
                });
            }
            "ime" => {
                let (kind, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                let ev = match kind {
                    "preedit" => Event::Ime(ImeEvent::Preedit {
                        text: text.to_string(),
                        active_range_chars: None,
                    }),
                    "commit" => Event::Ime(ImeEvent::Commit(text.to_string())),
                    other => return Err(err(&format!("未知 IME 动作 {other}"))),
                };
                steps.push(Step {
                    label: format!("ime {kind} {text:?}"),
                    events: vec![ev],
                    sleep: None,
                });
            }
            "click" | "doubleclick" | "rightclick" | "middleclick" | "sclick" => {
                let pos = parse_pos(rest).map_err(|e| err(&e))?;
                let times = if verb == "doubleclick" { 2 } else { 1 };
                let button = match verb {
                    "rightclick" => PointerButton::Secondary,
                    "middleclick" => PointerButton::Middle,
                    _ => PointerButton::Primary,
                };
                // `sclick` = 按住 Shift 点：扩选/缩选（`docs/28` §6.11 `card-selection`）。
                let mods = if verb == "sclick" {
                    let mut m = Modifiers::NONE;
                    m.shift = true;
                    m
                } else {
                    Modifiers::NONE
                };
                // 移入必须单独占一帧：egui 用上一帧登记的控件矩形做命中测试，
                // 同帧移入并按下会漏掉「指针刚进入控件」的点击（X20 narrow-menu
                // 归档脚本曾因此不可复现；先移动一帧后与归档图逐字节一致）。
                steps.push(move_step(pos));
                if mods.shift {
                    steps.push(Step {
                        label: "mods shift".into(),
                        events: vec![Event::ModifiersChanged(mods)],
                        sleep: None,
                    });
                }
                for _ in 0..times {
                    steps.push(pointer_step_mod(pos, button, true, mods));
                    steps.push(pointer_step_mod(pos, button, false, mods));
                }
                if mods.shift {
                    steps.push(Step {
                        label: "mods none".into(),
                        events: vec![Event::ModifiersChanged(Modifiers::NONE)],
                        sleep: None,
                    });
                }
            }
            "mods" => {
                // 显式立起/清掉修饰键：拖拽的 additive / subtract 是在**按下那一刻**
                // 就读走的（`sclick` 只够管单击），Ctrl+框选这类脚本必须先把状态摆好。
                let mods = parse_mods(rest).map_err(|e| err(&e))?;
                held_mods = mods;
                let any = mods.shift || mods.alt || mods.command;
                steps.push(Step {
                    label: format!("mods {}", if any { rest } else { "none" }),
                    events: vec![Event::ModifiersChanged(mods)],
                    sleep: None,
                });
            }
            "press" | "release" => {
                // 可选按钮前缀：`press middle 900 600`。不写就是主键，老脚本一个字都不用改。
                // 中键拖拽是画布平移的真实手势（空格拖等价，但键按住没法在脚本里跨帧表达）。
                let (button, rest) = match rest.split_once(char::is_whitespace) {
                    Some((head, tail)) => match head {
                        "primary" => (PointerButton::Primary, tail),
                        "middle" => (PointerButton::Middle, tail),
                        "secondary" => (PointerButton::Secondary, tail),
                        _ => (PointerButton::Primary, rest),
                    },
                    None => (PointerButton::Primary, rest),
                };
                let pos = parse_pos(rest).map_err(|e| err(&e))?;
                steps.push(pointer_step(pos, button, verb == "press"));
            }
            "move" => {
                let pos = parse_pos(rest).map_err(|e| err(&e))?;
                steps.push(move_step(pos));
            }
            "hover" => {
                let (pos, hold_ms) = parse_hover(rest).map_err(|e| err(&e))?;
                steps.push(Step {
                    label: format!("hover {pos:?} hold={hold_ms}ms"),
                    events: vec![Event::PointerMoved(pos)],
                    sleep: None,
                });
                if hold_ms > 0 {
                    steps.push(Step {
                        label: format!("hover_hold {hold_ms}ms"),
                        events: Vec::new(),
                        sleep: Some(Duration::from_millis(hold_ms)),
                    });
                }
            }
            "scroll" | "scroll_lines" => {
                let (dx, dy) = parse_pair(rest).map_err(|e| err(&e))?;
                // 两条原语对应两块真实硬件：触控板是 Point，滚轮是 Line（一格 ±1）。
                let unit = if verb == "scroll_lines" {
                    MouseWheelUnit::Line
                } else {
                    MouseWheelUnit::Point
                };
                steps.push(Step {
                    label: format!("{verb} dx={dx} dy={dy}"),
                    events: vec![Event::MouseWheel {
                        unit,
                        delta: Vec2::new(dx, dy),
                        phase: TouchPhase::Move,
                        source: MouseWheelSource::Wheel,
                        // 只认 `mods` 行摆好的状态：没有 `mods` 的老脚本仍是全 false，
                        // 手感与归档脚本逐帧一致（不重拍任何既有基线）。
                        modifiers: held_mods,
                    }],
                    sleep: None,
                });
            }
            "paste" => {
                steps.push(Step {
                    label: format!("paste {rest:?}"),
                    events: vec![Event::Paste(unescape(rest))],
                    sleep: None,
                });
            }
            "key_paste" => {
                let (chord, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                let (mods, name) = split_key(chord);
                let key = name
                    .and_then(Key::from_name)
                    .ok_or_else(|| err("认不出这个键名"))?;
                steps.push(Step {
                    label: format!("key_paste {chord} {text:?}"),
                    events: vec![
                        Event::ModifiersChanged(mods),
                        Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: mods,
                        },
                        Event::Paste(unescape(text)),
                    ],
                    sleep: None,
                });
                steps.push(Step {
                    label: format!("release {}", key.name()),
                    events: vec![
                        Event::Key {
                            key,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: mods,
                        },
                        Event::ModifiersChanged(Modifiers::NONE),
                    ],
                    sleep: None,
                });
            }
            "sleep" => {
                let ms: u64 = rest.parse().map_err(|_| err("的毫秒数不是数字"))?;
                steps.push(Step {
                    label: format!("sleep {ms}ms"),
                    events: Vec::new(),
                    sleep: Some(Duration::from_millis(ms)),
                });
            }
            other => return Err(err(&format!("未知动作 {other}"))),
        }
    }
    Ok(steps)
}

fn pointer_step(pos: Pos2, button: PointerButton, pressed: bool) -> Step {
    pointer_step_mod(pos, button, pressed, Modifiers::NONE)
}

fn pointer_step_mod(pos: Pos2, button: PointerButton, pressed: bool, mods: Modifiers) -> Step {
    Step {
        label: format!("pointer {pos:?} {button:?} pressed={pressed}"),
        sleep: None,
        events: vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: mods,
            },
        ],
    }
}

/// 只移动指针的一帧，与按键帧分开。
fn move_step(pos: Pos2) -> Step {
    Step {
        label: format!("move {pos:?}"),
        events: vec![Event::PointerMoved(pos)],
        sleep: None,
    }
}

/// 脚本里的 `\n` / `\t` / `\s`（空格）/ `\\` 转义；其余反斜杠原样保留。
fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// `mods ctrl` / `mods shift alt` / `mods none`：显式设置修饰键状态。
///
/// 拖拽的 `additive` / `subtract` 在按下那一刻就从 `ui.input().modifiers` 读走了，
/// 所以要脚本化「Ctrl+框选」必须在 `press` 之前把状态摆好；`sclick` 只覆盖单击。
fn parse_mods(raw: &str) -> Result<Modifiers, String> {
    let mut m = Modifiers::NONE;
    let raw = raw.trim();
    if raw.is_empty() || raw == "none" {
        return Ok(m);
    }
    for token in raw.split_whitespace() {
        match token {
            "ctrl" | "control" | "command" => m.command = true,
            "shift" => m.shift = true,
            "alt" => m.alt = true,
            other => return Err(format!("未知修饰键 {other}")),
        }
    }
    Ok(m)
}

fn parse_pos(raw: &str) -> Result<Pos2, String> {
    let (x, y) = raw
        .split_once([' ', ','])
        .ok_or_else(|| "指针动作缺坐标".to_string())?;
    let x: f32 = x.trim().parse().map_err(|_| "x 不是数字".to_string())?;
    let y: f32 = y.trim().parse().map_err(|_| "y 不是数字".to_string())?;
    Ok(Pos2::new(x, y))
}

fn parse_hover(raw: &str) -> Result<(Pos2, u64), String> {
    let fields: Vec<&str> = raw.split_whitespace().collect();
    if fields.len() < 2 || fields.len() > 3 {
        return Err("hover 需要 x y [hold_ms]".into());
    }
    let pos = parse_pos(&format!("{} {}", fields[0], fields[1]))?;
    let hold = fields
        .get(2)
        .map(|v| v.parse::<u64>())
        .transpose()
        .map_err(|_| "hover 的保持毫秒数不是数字".to_string())?
        .unwrap_or(0);
    Ok((pos, hold))
}

fn parse_pair(raw: &str) -> Result<(f32, f32), String> {
    let fields: Vec<&str> = raw.split_whitespace().collect();
    if fields.len() != 2 {
        return Err("需要两个数字".into());
    }
    let x: f32 = fields[0]
        .parse()
        .map_err(|_| "第一个数不是数字".to_string())?;
    let y: f32 = fields[1]
        .parse()
        .map_err(|_| "第二个数不是数字".to_string())?;
    Ok((x, y))
}

/// `ctrl+shift+Enter` → 修饰键 + 键名。`ctrl` 同时置 `command`，与 winit 在
/// Windows 上给出的事件一致（egui 的 Ctrl 组合按 command 匹配）。
fn split_key(raw: &str) -> (Modifiers, Option<&str>) {
    let mut mods = Modifiers::NONE;
    let mut name = raw;
    while let Some((head, tail)) = name.split_once('+') {
        match head.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                mods.ctrl = true;
                mods.command = true;
            }
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            _ => break,
        }
        name = tail;
    }
    (mods, Some(name).filter(|s| !s.is_empty()))
}

fn mods_label(mods: Modifiers) -> String {
    let mut out = String::new();
    if mods.ctrl {
        out.push_str("ctrl+");
    }
    if mods.shift {
        out.push_str("shift+");
    }
    if mods.alt {
        out.push_str("alt+");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{parse, split_key, GoldenInput};
    use eframe::egui::{Key, Modifiers, MouseWheelUnit, RawInput};
    use std::time::Duration;

    fn input(raw: &str) -> GoldenInput {
        GoldenInput {
            steps: parse(raw).unwrap(),
            next: 0,
            applied: 0,
            pointer: false,
            sleep_until: None,
            done_traced: false,
        }
    }

    /// 打磨轮 35：金样时间必须由帧数决定。`sleep` 期间时钟冻结（不截帧），
    /// 到期那一帧补上脚本写的名义时长——慢机器也停在同一个虚拟时刻。
    #[test]
    fn clock_is_frame_based_and_sleep_adds_nominal_time() {
        let mut g = input("sleep 40\nwait 1\n");
        assert!((g.advance_clock() - 1.0 / 60.0).abs() < 1e-9);
        let mut raw = RawInput::default();
        g.apply(&mut raw);
        assert!(g.sleep_until.is_some(), "sleep 行应该立起睡眠");
        assert_eq!(g.advance_clock(), 0.0, "睡眠期间时钟冻结");
        std::thread::sleep(Duration::from_millis(60));
        let adv = g.advance_clock();
        assert!((adv - 0.040).abs() < 1e-9, "到期补名义时长，实际 {adv}");
        assert!(g.sleep_until.is_none());
        g.apply(&mut raw);
        assert!(g.finished(), "续上 wait 1 后脚本结束");
    }

    /// X21：`mods ctrl` 之后 `scroll` 的事件必须**自带** ctrl。
    ///
    /// egui 判「Ctrl+滚轮 = 缩放」读的是 `Event::MouseWheel.modifiers`
    /// （`WheelState::modifiers` → `zoom_modifier` 的 `matches_any`），不是
    /// `InputState.modifiers`。这里以前把事件修饰键写死成 `NONE`，于是 `mods ctrl` +
    /// `scroll` 只能平移——画布缩放的 golden（`mm-zoom-wheel`）拍不出来。
    /// 没有 `mods` 行的老脚本保持全 false：向后兼容，不重拍既有基线。
    #[test]
    fn scroll_carries_the_held_modifiers_for_ctrl_wheel_zoom() {
        let steps = parse(
            "mods ctrl
scroll_lines 0 -3
mods none
scroll 0 -240
",
        )
        .unwrap();
        let wheel = |i: usize| match &steps[i].events[0] {
            eframe::egui::Event::MouseWheel {
                modifiers, unit, ..
            } => (*modifiers, *unit),
            other => panic!("第 {i} 步不是滚轮事件：{other:?}"),
        };
        let (mods, unit) = wheel(1);
        assert!(
            mods.ctrl || mods.command,
            "Ctrl 必须跟着滚轮事件走（egui 只认事件自带的修饰键）：{mods:?}"
        );
        assert_eq!(unit, MouseWheelUnit::Line, "scroll_lines 走 Line 单元");
        let (mods, unit) = wheel(3);
        assert!(
            !mods.ctrl && !mods.command && !mods.shift && !mods.alt,
            "`mods none` 之后要清掉：{mods:?}"
        );
        assert_eq!(unit, MouseWheelUnit::Point, "scroll 走 Point 单元");

        // 老脚本（一行 `mods` 都没有）仍是全 false。
        let steps = parse(
            "scroll 0 -240
",
        )
        .unwrap();
        match &steps[0].events[0] {
            eframe::egui::Event::MouseWheel { modifiers, .. } => {
                assert_eq!(*modifiers, Modifiers::NONE);
            }
            other => panic!("不是滚轮事件：{other:?}"),
        }
    }

    #[test]
    fn ctrl_sets_command_like_winit() {
        let (mods, name) = split_key("ctrl+shift+Enter");
        assert_eq!(name, Some("Enter"));
        assert!(mods.ctrl && mods.command && mods.shift);
        assert_eq!(Key::from_name(name.unwrap()), Some(Key::Enter));
    }

    #[test]
    fn wait_expands_and_clicks_move_before_pressing() {
        let steps =
            parse("wait 3\nclick 10 20\ndoubleclick 10 20\nrightclick 10 20\nmiddleclick 10 20\n")
                .unwrap();
        assert_eq!(steps.len(), 3 + (1 + 2) + (1 + 4) + (1 + 2) + (1 + 2));
        assert!(steps[0].events.is_empty());

        // 每个点击动作前面都单独有一帧 PointerMoved：egui 的命中测试读上一帧
        // 登记的控件矩形，同帧移入并按下会漏掉「指针刚进入控件」的点击。
        for (start, want) in [
            (3usize, eframe::egui::PointerButton::Primary),
            (6, eframe::egui::PointerButton::Primary),
            (11, eframe::egui::PointerButton::Secondary),
            (14, eframe::egui::PointerButton::Middle),
        ] {
            match steps[start].events.as_slice() {
                [eframe::egui::Event::PointerMoved(p)] => {
                    assert_eq!(*p, eframe::egui::pos2(10.0, 20.0));
                }
                other => panic!("点击前必须单独移动一帧：{other:?}"),
            }
            match steps[start + 1].events.as_slice() {
                [eframe::egui::Event::PointerMoved(_), eframe::egui::Event::PointerButton {
                    button, pressed, ..
                }] => {
                    assert_eq!(*button, want);
                    assert!(pressed, "{}", steps[start + 1].label);
                }
                other => panic!("{}", format!("{other:?}")),
            }
        }
    }

    #[test]
    fn press_can_name_its_button_and_defaults_to_primary() {
        use super::Step;
        use eframe::egui::{Event, PointerButton};
        let steps = parse("press 10 20\nrelease 10 20\npress middle 30 40\nrelease middle 30 40\n")
            .unwrap();
        let button_of = |s: &Step| match s.events.as_slice() {
            [Event::PointerMoved(_), Event::PointerButton {
                button, pressed, ..
            }] => (*button, *pressed),
            other => panic!("{other:?}"),
        };
        assert_eq!(button_of(&steps[0]), (PointerButton::Primary, true));
        assert_eq!(button_of(&steps[1]), (PointerButton::Primary, false));
        // 中键拖拽是画布平移的真实手势（空格拖等价，但按键没法在脚本里跨帧按住）。
        assert_eq!(button_of(&steps[2]), (PointerButton::Middle, true));
        assert_eq!(button_of(&steps[3]), (PointerButton::Middle, false));
    }

    /// 打磨轮 9：拖拽的 additive / subtract 在**按下那一刻**就读走，
    /// 要脚本化「Ctrl+框选」得先把修饰键立起来，`sclick` 只覆盖单击。
    #[test]
    fn mods_sets_and_clears_the_modifier_state() {
        use super::Step;
        use eframe::egui::Event;
        let steps = parse("mods ctrl\npress 10 20\nrelease 10 20\nmods none\n").unwrap();
        let mods_of = |s: &Step| match s.events.as_slice() {
            [Event::ModifiersChanged(m)] => Some(*m),
            _ => None,
        };
        let held = mods_of(&steps[0]).expect("第一行是 mods");
        assert!(held.command, "mods ctrl 要立起 command");
        assert!(!held.shift && !held.alt);
        // 按下/松开本身不带修饰键事件：egui 靠 ModifiersChanged 的状态维持。
        assert!(mods_of(&steps[1]).is_none());
        assert!(mods_of(&steps[2]).is_none());
        let cleared = mods_of(&steps[3]).expect("最后一行是 mods none");
        assert!(!cleared.command && !cleared.shift && !cleared.alt);
        // 拼错就报错，别静默当成没写。
        assert!(parse("mods ctrk\n").is_err());
    }

    #[test]
    fn shift_click_carries_the_shift_modifier() {
        let steps = parse("sclick 10 20\n").unwrap();
        // 移入 + 按下 Shift + 按下 + 松开 + 松开 Shift。
        assert_eq!(
            steps.len(),
            5,
            "sclick = move + mods + press + release + mods"
        );
        match steps[2].events.as_slice() {
            [eframe::egui::Event::PointerMoved(_), eframe::egui::Event::PointerButton {
                modifiers, pressed, ..
            }] => {
                assert!(modifiers.shift, "扩选靠 Shift，指针事件里必须带上");
                assert!(*pressed);
            }
            other => panic!("{other:?}"),
        }
        // 普通点击不带 Shift（对照组）。
        let plain = parse("click 10 20\n").unwrap();
        assert_eq!(plain.len(), 3, "普通点击没有 mods 帧");
    }

    #[test]
    fn bad_lines_are_reported_with_line_numbers() {
        let err = parse("key NotAKey\n").unwrap_err();
        assert!(err.contains("第 1 行"), "{err}");
    }

    #[test]
    fn text_keeps_spaces_after_the_verb() {
        let steps = parse("text 甲乙 丙\n").unwrap();
        match &steps[0].events[0] {
            eframe::egui::Event::Text(t) => assert_eq!(t, "甲乙 丙"),
            other => panic!("{other:?}"),
        }
        let mods = Modifiers::NONE;
        assert!(!mods.ctrl);
    }

    #[test]
    fn hover_can_hold_and_scroll_is_a_real_wheel_event() {
        let steps = parse("hover 12 34 700\nscroll 0 -240\n").unwrap();
        assert_eq!(steps.len(), 3, "hover = move + sleep, scroll = one frame");
        assert!(steps[1].sleep.is_some_and(|d| d.as_millis() == 700));
        match &steps[2].events[0] {
            eframe::egui::Event::MouseWheel { delta, unit, .. } => {
                assert_eq!(*delta, eframe::egui::Vec2::new(0.0, -240.0));
                assert_eq!(*unit, MouseWheelUnit::Point, "触控板路径");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn scroll_lines_emits_line_units_for_real_mice() {
        let steps = parse("scroll_lines 0 -3\n").unwrap();
        match &steps[0].events[0] {
            eframe::egui::Event::MouseWheel { delta, unit, .. } => {
                assert_eq!(*delta, eframe::egui::Vec2::new(0.0, -3.0));
                assert_eq!(*unit, MouseWheelUnit::Line, "滚轮一格是 Line，不是 Point");
            }
            other => panic!("{other:?}"),
        }
    }
}
