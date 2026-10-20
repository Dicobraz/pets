//! Проверка наборов для теста приложения: разбор, дубли, ключи и переменные по языкам.

use std::collections::{BTreeMap, BTreeSet};

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

use crate::{Problem, Resource, languages, parse};

/// Переменные сообщения: `$name` из значения и атрибутов.
type Vars = BTreeSet<String>;

/// Проверка наборов (для теста приложения): ошибки разбора, ключ есть в одном языке и нет в другом,
/// дубль ключа, разные переменные ($name) у одного ключа в разных языках. Пусто — наборы согласованы.
pub fn check(resources: &[&[Resource]]) -> Vec<Problem> {
    let mut problems = Vec::new();
    // Язык → сообщения и их переменные. Термы (`-brand`) — только на дубли: у языков бывают свои.
    let mut per_lang: Vec<(&str, BTreeMap<String, Vars>)> = Vec::new();
    for lang in languages(resources) {
        let mut messages = BTreeMap::new();
        let mut terms = BTreeSet::new();
        for &(l, source) in resources.iter().flat_map(|set| set.iter()) {
            if !l.eq_ignore_ascii_case(lang) {
                continue;
            }
            let resource = parse(lang, source, &mut problems);
            for entry in resource.entries() {
                let duplicate = match entry {
                    // При дубле действует первое определение, как в бандле.
                    Entry::Message(m) if messages.contains_key(m.id.name) => Some(m.id.name.to_owned()),
                    Entry::Message(m) => {
                        let mut vars = Vars::new();
                        if let Some(value) = &m.value {
                            pattern_vars(value, &mut vars);
                        }
                        for a in &m.attributes {
                            pattern_vars(&a.value, &mut vars);
                        }
                        messages.insert(m.id.name.to_owned(), vars);
                        None
                    }
                    Entry::Term(t) => {
                        let key = format!("-{}", t.id.name);
                        (!terms.insert(key.clone())).then_some(key)
                    }
                    _ => None,
                };
                if let Some(key) = duplicate {
                    problems.push(Problem::Duplicate {
                        lang: lang.to_owned(),
                        key,
                    });
                }
            }
        }
        per_lang.push((lang, messages));
    }

    let keys: BTreeSet<&String> = per_lang.iter().flat_map(|(_, m)| m.keys()).collect();
    for key in keys {
        let Some((base_lang, base_vars)) = per_lang.iter().find_map(|(l, m)| m.get(key).map(|v| (*l, v))) else {
            continue;
        };
        for (lang, messages) in &per_lang {
            match messages.get(key) {
                None => problems.push(Problem::Missing {
                    key: key.clone(),
                    present: base_lang.to_owned(),
                    missing: (*lang).to_owned(),
                }),
                Some(vars) if vars != base_vars => problems.push(Problem::Variables {
                    key: key.clone(),
                    lang: base_lang.to_owned(),
                    vars: show(base_vars),
                    other: (*lang).to_owned(),
                    other_vars: show(vars),
                }),
                Some(_) => {}
            }
        }
    }
    problems
}

/// `$a, $b`; пусто — «нет».
fn show(vars: &Vars) -> String {
    if vars.is_empty() {
        return "нет".to_owned();
    }
    vars.iter().map(|v| format!("${v}")).collect::<Vec<_>>().join(", ")
}

fn pattern_vars(pattern: &Pattern<&str>, out: &mut Vars) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_vars(expression, out);
        }
    }
}

fn expression_vars(expression: &Expression<&str>, out: &mut Vars) {
    match expression {
        Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                pattern_vars(&v.value, out);
            }
        }
        Expression::Inline(inline) => inline_vars(inline, out),
    }
}

fn inline_vars(inline: &InlineExpression<&str>, out: &mut Vars) {
    match inline {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. }
        | InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for a in &arguments.positional {
                inline_vars(a, out);
            }
            for a in &arguments.named {
                inline_vars(&a.value, out);
            }
        }
        InlineExpression::Placeable { expression } => expression_vars(expression, out),
        _ => {}
    }
}
