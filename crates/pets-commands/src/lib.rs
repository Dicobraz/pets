//! Реестр команд приложения (подход «действия пользователя»): каждое действие — команда с
//! идентификатором, сочетаниями клавиш по умолчанию и контекстом, где они действуют. Из реестра
//! строятся привязки клавиш, кнопки и подсказки, внешний вызов по id и чек-лист соответствия.
//!
//! Крейт без UI. Записи клавиш и контекстов — в синтаксисе gpui (`ctrl-shift-r`, `Mailer && !Input`):
//! интерфейс всех приложений Pets — gpui. Привязка к действиям gpui — в приложении (позже `pets-ui`).

use std::collections::HashSet;
use std::fmt::Write as _;

/// Одна команда приложения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// `область.действие`, строчными латинскими буквами, цифрами и `_`: `mail.reply_all`, `view.calendar`.
    pub id: &'static str,
    /// Сочетания по умолчанию в записи gpui: `ctrl-shift-r`, `j`, `pagedown`. Пусто — только из меню.
    pub keys: &'static [&'static str],
    /// Где действуют клавиши — выражение контекста gpui (`Mailer && !Input`); `None` — во всём окне.
    pub context: Option<&'static str>,
}

impl Command {
    /// Область команды: `mail` у `mail.reply`.
    pub fn area(&self) -> &'static str {
        self.id.split_once('.').map_or(self.id, |(a, _)| a)
    }

    /// Ключ названия в Fluent: `mail.reply_all` → `cmd-mail-reply-all`.
    pub fn name_key(&self) -> String {
        format!("cmd-{}", self.id.replace(['.', '_'], "-"))
    }

    /// Имя действия gpui: `mail.reply_all` → `mail::ReplyAll`. Приложение объявляет действия
    /// `actions!(mail, [ReplyAll, …])` и находит их по этому имени.
    pub fn action_name(&self) -> String {
        let (area, verb) = self.id.split_once('.').unwrap_or((self.id, ""));
        let mut name = format!("{area}::");
        for part in verb.split('_') {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                name.extend(first.to_uppercase());
                name.push_str(chars.as_str());
            }
        }
        name
    }
}

/// Ошибка в таблице команд.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("{0}: id должен быть «область.действие» из [a-z0-9_]")]
    BadId(&'static str),
    #[error("{0}: id повторяется")]
    DuplicateId(&'static str),
    #[error("{id}: сочетание «{keys}» не разобрано: {why}")]
    BadKeys {
        id: &'static str,
        keys: &'static str,
        why: String,
    },
    #[error("«{keys}» в контексте {context:?} — у {first} и {second}")]
    KeyConflict {
        keys: String,
        context: Option<&'static str>,
        first: &'static str,
        second: &'static str,
    },
}

/// Модификаторы gpui.
const MODIFIERS: &[&str] = &["ctrl", "alt", "shift", "win", "cmd", "super", "fn", "secondary"];

/// Именованные клавиши gpui (кроме `f1`…`f24`).
const NAMED_KEYS: &[&str] = &[
    "space",
    "backspace",
    "enter",
    "tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "escape",
    "insert",
    "delete",
    "menu",
    "back",
    "forward",
];

/// Проверить одно сочетание; вернуть каноническую запись (модификаторы по алфавиту) для поиска конфликтов.
/// Последовательности через пробел (`g g`) разбираются по частям.
pub fn parse_keys(keys: &str) -> Result<String, String> {
    if keys.trim().is_empty() {
        return Err("пусто".into());
    }
    let mut out = Vec::new();
    for stroke in keys.split(' ').filter(|s| !s.is_empty()) {
        let mut parts: Vec<&str> = stroke.split('-').collect();
        // `ctrl--` — клавиша «минус»: последний пустой кусок значит, что ключ — «-».
        let key = match parts.pop() {
            Some("") if parts.last() == Some(&"") => {
                parts.pop();
                "-"
            }
            Some(k) => k,
            None => return Err(format!("«{stroke}»: нет клавиши")),
        };
        let mut mods: Vec<&str> = Vec::new();
        for m in parts {
            if !MODIFIERS.contains(&m) {
                return Err(format!("«{m}» — не модификатор"));
            }
            if mods.contains(&m) {
                return Err(format!("«{m}» дважды"));
            }
            mods.push(m);
        }
        let single = key.chars().count() == 1 && !key.chars().any(|c| c.is_uppercase() || c.is_whitespace());
        let function = key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n));
        if !(single || function || NAMED_KEYS.contains(&key)) {
            return Err(format!("«{key}» — неизвестная клавиша (буквы — строчные)"));
        }
        mods.sort_unstable();
        let mut s = String::new();
        for m in &mods {
            let _ = write!(s, "{m}-");
        }
        s.push_str(key);
        out.push(s);
    }
    Ok(out.join(" "))
}

fn valid_id(id: &str) -> bool {
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    matches!(id.split_once('.'), Some((a, v)) if ok(a) && ok(v))
}

/// Проверить таблицу команд: id, сочетания, повторы и конфликты клавиш в одном контексте.
/// Пустой результат — таблица корректна.
pub fn validate(commands: &[Command]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut ids = HashSet::new();
    let mut bound: Vec<(String, Option<&'static str>, &'static str)> = Vec::new();
    for c in commands {
        if !valid_id(c.id) {
            problems.push(Problem::BadId(c.id));
        }
        if !ids.insert(c.id) {
            problems.push(Problem::DuplicateId(c.id));
        }
        for &keys in c.keys {
            match parse_keys(keys) {
                Err(why) => problems.push(Problem::BadKeys { id: c.id, keys, why }),
                Ok(canon) => {
                    if let Some(&(_, _, first)) = bound.iter().find(|(k, ctx, _)| *k == canon && *ctx == c.context) {
                        problems.push(Problem::KeyConflict {
                            keys: canon.clone(),
                            context: c.context,
                            first,
                            second: c.id,
                        });
                    }
                    bound.push((canon, c.context, c.id));
                }
            }
        }
    }
    problems
}

/// Команда по id.
pub fn find<'a>(commands: &'a [Command], id: &str) -> Option<&'a Command> {
    commands.iter().find(|c| c.id == id)
}

/// Команды, которым назначено сочетание (в канонической записи); для проверок вроде «хоткеи Outlook привязаны».
pub fn bound_to<'a>(commands: &'a [Command], keys: &str) -> Vec<&'a Command> {
    let Ok(want) = parse_keys(keys) else { return Vec::new() };
    commands
        .iter()
        .filter(|c| c.keys.iter().any(|k| parse_keys(k).is_ok_and(|k| k == want)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn cmd(id: &'static str, keys: &'static [&'static str], context: Option<&'static str>) -> Command {
        Command { id, keys, context }
    }

    #[test]
    fn names_follow_the_id() {
        let c = cmd("mail.reply_all", &["ctrl-shift-r"], None);
        assert_eq!(c.area(), "mail");
        assert_eq!(c.name_key(), "cmd-mail-reply-all");
        assert_eq!(c.action_name(), "mail::ReplyAll");
        assert_eq!(cmd("view.calendar", &[], None).action_name(), "view::Calendar");
    }

    #[test]
    fn keys_are_parsed_and_made_canonical() {
        assert_eq!(parse_keys("shift-ctrl-r"), Ok("ctrl-shift-r".into()));
        assert_eq!(parse_keys("pagedown"), Ok("pagedown".into()));
        assert_eq!(parse_keys("f12"), Ok("f12".into()));
        assert_eq!(parse_keys("ctrl--"), Ok("ctrl--".into()));
        assert_eq!(parse_keys("g g"), Ok("g g".into()));
        assert!(parse_keys("ctrl-R").is_err(), "буквы — строчные");
        assert!(parse_keys("hyper-x").is_err());
        assert!(parse_keys("ctrl-ctrl-x").is_err());
        assert!(parse_keys("pgdn").is_err());
        assert!(parse_keys("f25").is_err());
        assert!(parse_keys("").is_err());
    }

    #[test]
    fn a_good_table_has_no_problems() {
        let t = [
            cmd("mail.next", &["j", "down"], Some("Mailer && !Input")),
            cmd("mail.reply", &["ctrl-r"], Some("Mailer")),
            cmd("search.focus", &["ctrl-e"], None),
        ];
        assert_eq!(validate(&t), vec![]);
        assert_eq!(find(&t, "mail.reply").map(|c| c.keys), Some(&["ctrl-r"][..]));
        assert_eq!(bound_to(&t, "j").len(), 1);
    }

    #[test]
    fn problems_are_reported() {
        let t = [
            cmd("Mail.next", &[], None),
            cmd("mail.next", &["j"], Some("A")),
            cmd("mail.next", &[], None),
            cmd("mail.prev", &["j"], Some("A")),
            cmd("mail.down", &["j"], Some("B")),
            cmd("mail.bad", &["ctrl-"], None),
        ];
        let p = validate(&t);
        assert!(p.contains(&Problem::BadId("Mail.next")));
        assert!(p.contains(&Problem::DuplicateId("mail.next")));
        assert!(p.iter().any(|p| matches!(
            p,
            Problem::KeyConflict {
                first: "mail.next",
                second: "mail.prev",
                ..
            }
        )));
        assert!(
            !p.iter().any(|p| matches!(
                p,
                Problem::KeyConflict {
                    second: "mail.down",
                    ..
                }
            )),
            "другой контекст"
        );
        assert!(p.iter().any(|p| matches!(p, Problem::BadKeys { id: "mail.bad", .. })));
    }
}
