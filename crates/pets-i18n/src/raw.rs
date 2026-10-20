//! Сырые строки интерфейса в исходнике Rust: маленький токенизатор литералов и комментариев.

use std::collections::HashSet;

/// Строковый литерал с кириллицей: строка исходника (с 1) и текст между кавычками как в исходнике.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawString {
    pub line: usize,
    pub text: String,
}

/// Отметка «здесь можно»: строка исходника с ней пропускается.
const OK_MARK: &str = "// i18n-ok";

/// Сырые строки интерфейса в исходнике Rust: строковые литералы с кириллицей вне комментариев.
/// Сканирование останавливается на первом `#[cfg(test)]` в начале строки (тесты в конце файла).
/// Строка с комментарием `// i18n-ok` пропускается.
pub fn raw_strings(source: &str) -> Vec<RawString> {
    let ok: HashSet<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains(OK_MARK))
        .map(|(i, _)| i + 1)
        .collect();
    let mut lx = Lexer {
        s: source.chars().collect(),
        i: 0,
        line: 1,
    };
    let mut out = Vec::new();
    let mut report = |line: usize, text: String| {
        if text.chars().any(cyrillic) && !ok.contains(&line) {
            out.push(RawString { line, text });
        }
    };
    while let Some(c) = lx.peek(0) {
        if lx.at_line_start() && lx.starts_with("#[cfg(test)]") {
            break;
        }
        let line = lx.line;
        match c {
            '/' if lx.peek(1) == Some('/') => lx.skip_line(),
            '/' if lx.peek(1) == Some('*') => lx.skip_block(),
            '"' => {
                lx.bump();
                report(line, lx.string());
            }
            '\'' => lx.char_or_lifetime(),
            c if ident(c) => {
                let word = lx.word();
                match word.as_str() {
                    "r" | "br" | "cr" if lx.raw_start() => {
                        let text = lx.raw_string();
                        if word == "r" {
                            report(line, text);
                        }
                    }
                    // Байтовые и C-строки — не текст интерфейса.
                    "b" | "c" if lx.peek(0) == Some('"') => {
                        lx.bump();
                        lx.string();
                    }
                    _ => {}
                }
            }
            _ => lx.bump(),
        }
    }
    out
}

fn cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Lexer {
    s: Vec<char>,
    i: usize,
    /// Текущая строка, с 1.
    line: usize,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.s.get(self.i + ahead).copied()
    }

    fn bump(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.i += 1;
    }

    fn at_line_start(&self) -> bool {
        self.i == 0 || self.s.get(self.i - 1) == Some(&'\n')
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars().enumerate().all(|(k, c)| self.peek(k) == Some(c))
    }

    /// `//…` до конца строки, перевод строки не съедается.
    fn skip_line(&mut self) {
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.bump();
        }
    }

    /// `/* … */` с вложенностью.
    fn skip_block(&mut self) {
        self.i += 2;
        let mut depth = 1;
        while depth > 0 && self.peek(0).is_some() {
            if self.starts_with("/*") {
                depth += 1;
                self.i += 2;
            } else if self.starts_with("*/") {
                depth -= 1;
                self.i += 2;
            } else {
                self.bump();
            }
        }
    }

    /// Тело обычной строки после открывающей кавычки, с экранированием; текст — как в исходнике.
    fn string(&mut self) -> String {
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            match c {
                '"' => {
                    self.bump();
                    break;
                }
                '\\' => {
                    text.push(c);
                    self.bump();
                    if let Some(escaped) = self.peek(0) {
                        text.push(escaped);
                        self.bump();
                    }
                }
                _ => {
                    text.push(c);
                    self.bump();
                }
            }
        }
        text
    }

    /// После префикса `r`: `#…#"` — начало сырой строки (иначе сырой идентификатор `r#type`).
    fn raw_start(&self) -> bool {
        let hashes = (0..).take_while(|&k| self.peek(k) == Some('#')).count();
        self.peek(hashes) == Some('"')
    }

    /// Сырая строка `#…#"…"#…#`, курсор на первом `#` или `"`.
    fn raw_string(&mut self) -> String {
        let hashes = (0..).take_while(|&k| self.peek(k) == Some('#')).count();
        self.i += hashes + 1;
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            if c == '"' && (1..=hashes).all(|k| self.peek(k) == Some('#')) {
                self.i += hashes + 1;
                break;
            }
            text.push(c);
            self.bump();
        }
        text
    }

    /// `'x'`, `'\n'`, `'\u{41}'` — символ; `'a` — время жизни или метка (имя съест `word`).
    fn char_or_lifetime(&mut self) {
        if self.peek(1) == Some('\\') {
            self.i += 3;
            while self.peek(0).is_some_and(|c| c != '\'' && c != '\n') {
                self.i += 1;
            }
            if self.peek(0) == Some('\'') {
                self.i += 1;
            }
        } else if self.peek(2) == Some('\'') && self.peek(1) != Some('\n') {
            self.i += 3;
        } else {
            self.i += 1;
        }
    }

    fn word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self.peek(0).filter(|&c| ident(c)) {
            word.push(c);
            self.i += 1;
        }
        word
    }
}
