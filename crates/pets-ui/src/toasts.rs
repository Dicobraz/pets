//! Тосты и история уведомлений.
//!
//! Логика очереди ([`Queue`]) — без окна и таймеров, проверяется тестами; [`Toasts`] — view gpui поверх неё.
//! Таймер скрытия живёт, только пока тост на экране: в простое кадров нет (кадры по запросу).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::*;
use pets_i18n::tr;

/// Важность уведомления: цвет метки и время на экране.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

impl Level {
    /// Сколько тост виден: ошибку успевают прочитать.
    pub fn duration(self) -> Duration {
        match self {
            Level::Error => Duration::from_secs(8),
            Level::Warning => Duration::from_secs(6),
            Level::Info | Level::Success => Duration::from_secs(4),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Level::Info => "ℹ",
            Level::Success => "✓",
            Level::Warning => "⚠",
            Level::Error => "✕",
        }
    }

    /// Цвета Fluent: info, success, warning, danger.
    fn color(self) -> Rgba {
        match self {
            Level::Info => rgb(0x0f6cbd),
            Level::Success => rgb(0x107c10),
            Level::Warning => rgb(0xbc4b09),
            Level::Error => rgb(0xc50f1f),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub level: Level,
    pub text: SharedString,
    pub at: Instant,
}

/// Тосты на экране (не больше [`Queue::MAX_SHOWN`], старый уходит) и история (новые сверху).
#[derive(Debug, Default)]
pub struct Queue {
    shown: Vec<Toast>,
    history: VecDeque<Toast>,
    unread: usize,
    next: u64,
}

impl Queue {
    pub const MAX_SHOWN: usize = 3;
    pub const MAX_HISTORY: usize = 100;

    /// Новый тост; возвращает его id для скрытия по таймеру.
    pub fn push(&mut self, level: Level, text: SharedString, at: Instant) -> u64 {
        self.next += 1;
        let toast = Toast {
            id: self.next,
            level,
            text,
            at,
        };
        self.history.push_front(toast.clone());
        self.history.truncate(Self::MAX_HISTORY);
        self.unread = (self.unread + 1).min(Self::MAX_HISTORY);
        self.shown.push(toast);
        if self.shown.len() > Self::MAX_SHOWN {
            self.shown.remove(0);
        }
        self.next
    }

    /// Убрать тост с экрана (в истории остаётся). `false` — его уже нет.
    pub fn hide(&mut self, id: u64) -> bool {
        let before = self.shown.len();
        self.shown.retain(|t| t.id != id);
        self.shown.len() != before
    }

    pub fn shown(&self) -> &[Toast] {
        &self.shown
    }

    /// История, новые сверху.
    pub fn history(&self) -> impl Iterator<Item = &Toast> {
        self.history.iter()
    }

    /// Уведомлений с последнего открытия истории.
    pub fn unread(&self) -> usize {
        self.unread
    }

    /// История просмотрена: тосты на экране больше не нужны.
    pub fn mark_read(&mut self) {
        self.unread = 0;
        self.shown.clear();
    }
}

/// Тосты окна. Встраивается последним ребёнком корня с `.relative()`: рисуется поверх, фокус не берёт.
#[derive(Default)]
pub struct Toasts {
    queue: Queue,
    history_open: bool,
}

impl Toasts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Показать уведомление; скроется само через [`Level::duration`].
    pub fn push(&mut self, level: Level, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.queue.push(level, text.into(), Instant::now());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(level.duration()).await;
            let _ = this.update(cx, |t, cx| {
                if t.queue.hide(id) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Открыть или закрыть историю уведомлений.
    pub fn toggle_history(&mut self, cx: &mut Context<Self>) {
        self.history_open = !self.history_open;
        if self.history_open {
            self.queue.mark_read();
        }
        cx.notify();
    }

    pub fn is_history_open(&self) -> bool {
        self.history_open
    }

    /// Для значка в строке состояния.
    pub fn unread(&self) -> usize {
        self.queue.unread()
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

const WIDTH: f32 = 360.;

impl Render for Toasts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.history_open {
            return history(&self.queue, cx).into_any_element();
        }
        if self.queue.shown().is_empty() {
            return div().into_any_element();
        }
        div()
            .absolute()
            .top(px(48.))
            .right(px(12.))
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap_2()
            .children(self.queue.shown().iter().rev().map(|t| {
                let id = t.id;
                card(t)
                    .id(("toast", id as usize))
                    .occlude()
                    .cursor_pointer()
                    .shadow_md()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.queue.hide(id) {
                            cx.notify();
                        }
                    }))
            }))
            .into_any_element()
    }
}

fn card(t: &Toast) -> Div {
    // Цвет у gpui — один на все стороны рамки: метка важности — полоса слева.
    div()
        .relative()
        .overflow_hidden()
        .flex()
        .gap_2()
        .p_3()
        .pl_4()
        .rounded_md()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xe0e0e0))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(4.))
                .bg(t.level.color()),
        )
        .child(div().text_color(t.level.color()).child(t.level.icon()))
        .child(div().flex_1().min_w_0().child(t.text.clone()))
}

fn history(queue: &Queue, cx: &mut Context<Toasts>) -> impl IntoElement {
    let now = Instant::now();
    let header = div()
        .flex()
        .items_center()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(0xe0e0e0))
        .child(
            div()
                .flex_1()
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr!("pets-ui-notifications")),
        )
        .child(
            div()
                .id("toasts-close")
                .px_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0xececec)))
                .child("✕")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_history(cx))),
        );
    let mut list = div()
        .id("toasts-history")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .p_2()
        .gap_2()
        .overflow_y_scroll();
    let mut empty = true;
    for t in queue.history() {
        empty = false;
        let min = now.duration_since(t.at).as_secs() / 60;
        list = list.child(
            card(t).child(
                div()
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .child(tr!("pets-ui-ago", min = min as i64)),
            ),
        );
    }
    if empty {
        list = list.child(
            div()
                .p_2()
                .text_color(rgb(0x888888))
                .child(tr!("pets-ui-notifications-empty")),
        );
    }
    div()
        .id("toasts-panel")
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(WIDTH))
        .flex()
        .flex_col()
        .occlude()
        .bg(rgb(0xfafafa))
        .border_l_1()
        .border_color(rgb(0xdddddd))
        .shadow_lg()
        .child(header)
        .child(list)
}

#[cfg(test)]
mod tests {
    use super::{Level, Queue};
    use std::time::Instant;

    #[test]
    fn at_most_three_are_shown_and_all_go_to_history() {
        let mut q = Queue::default();
        let now = Instant::now();
        let ids: Vec<u64> = (0..5)
            .map(|i| q.push(Level::Info, format!("t{i}").into(), now))
            .collect();
        assert_eq!(q.shown().iter().map(|t| t.id).collect::<Vec<_>>(), ids[2..]);
        assert_eq!(
            q.history().map(|t| t.id).collect::<Vec<_>>(),
            ids.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(q.unread(), 5);
    }

    #[test]
    fn hide_removes_from_screen_only() {
        let mut q = Queue::default();
        let id = q.push(Level::Error, "e".into(), Instant::now());
        assert!(q.hide(id));
        assert!(!q.hide(id));
        assert!(q.shown().is_empty());
        assert_eq!(q.history().count(), 1);
    }

    #[test]
    fn opening_history_marks_read() {
        let mut q = Queue::default();
        q.push(Level::Warning, "w".into(), Instant::now());
        q.mark_read();
        assert_eq!(q.unread(), 0);
        assert!(q.shown().is_empty());
        assert_eq!(q.history().count(), 1);
    }

    #[test]
    fn history_is_capped() {
        let mut q = Queue::default();
        for _ in 0..Queue::MAX_HISTORY + 10 {
            q.push(Level::Info, "x".into(), Instant::now());
        }
        assert_eq!(q.history().count(), Queue::MAX_HISTORY);
    }

    #[test]
    fn errors_stay_longer() {
        assert!(Level::Error.duration() > Level::Info.duration());
    }

    #[test]
    fn texts_exist_in_both_languages() {
        assert_eq!(pets_i18n::check(&[super::super::FTL]), vec![]);
    }
}
