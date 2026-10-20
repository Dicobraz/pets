#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Единственный экземпляр на Windows: второй вызов в том же процессе (mutex уже есть) передаёт аргументы первому.
#![cfg(windows)]

use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pets_shell::{Instance, single_instance};

/// Своё имя на прогон: тесты и параллельные прогоны не мешают друг другу.
fn unique_app(tag: &str) -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("PetsShellTest.{tag}.{}.{nanos}", std::process::id())
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn second_passes_args_to_first() {
    let app = unique_app("listen");
    let Instance::First(listener) = single_instance(&app, &[]).unwrap() else {
        panic!("первый вызов — не First");
    };
    let (tx, rx) = mpsc::channel();
    listener.listen(move |args| tx.send(args).unwrap());

    let args = strings(&["--open", r"C:\Письма\отчёт.eml"]);
    assert!(matches!(single_instance(&app, &args).unwrap(), Instance::Second));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), args);

    // Тот же экземпляр канала обслуживает следующих; пустые аргументы тоже доходят.
    assert!(matches!(single_instance(&app, &[]).unwrap(), Instance::Second));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Vec::<String>::new());
    let third = strings(&["mailto:a@b.c"]);
    assert!(matches!(single_instance(&app, &third).unwrap(), Instance::Second));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), third);
}

#[test]
fn args_sent_before_listen_are_kept() {
    let app = unique_app("early");
    let Instance::First(listener) = single_instance(&app, &[]).unwrap() else {
        panic!("первый вызов — не First");
    };
    // Второй пишет и закрывается раньше, чем первый начал слушать.
    let args = strings(&["early"]);
    assert!(matches!(single_instance(&app, &args).unwrap(), Instance::Second));

    let (tx, rx) = mpsc::channel();
    listener.listen(move |args| tx.send(args).unwrap());
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), args);
}

#[test]
fn bad_name_is_rejected() {
    assert!(single_instance("", &[]).is_err());
    assert!(single_instance(r"a\b", &[]).is_err());
}
