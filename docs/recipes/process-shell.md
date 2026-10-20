# Оболочка процесса: журнал, паники, единственный экземпляр

**Применимость:** любое приложение, Windows (не Windows — журнал в stderr, экземпляр всегда первый).

**Проблема.** У настольного приложения без консоли ошибки и паники пропадают: stderr никто не видит.
Повторный запуск (ярлык, `mailto:`, «Открыть с помощью») поднимает второе окно со своим состоянием вместо
того, чтобы отдать аргументы уже открытому и вывести его вперёд.

**Решение** — крейт `pets-shell`, без UI:

```rust
pets_shell::init_logging("Postman", "POSTMAN_LOG");
match pets_shell::single_instance("Postman", &args) {
    Ok(Instance::Second) => return,                          // аргументы у первого, код 0
    Ok(Instance::First(l)) => l.listen(move |a| tx.send(a)), // поток канала → UI через канал
    Err(e) => tracing::warn!("единственный экземпляр не проверен: {e}"), // работаем как первый
}
let profile = open_profile().unwrap_or_else(|e| pets_shell::fatal("профиль", e));
```

- Журнал: `tracing` в stderr, фильтр — `filter::Targets` из переменной (`info,gpui=warn`, по умолчанию
  `info`); записи `log` (gpui) — через tracing-log. Слой `tracing_subscriber::Layer` пишет события ERROR
  (сообщение и поля одной строкой) в журнал Windows «Приложение», источник — имя приложения, код 1000.
- Хук паники: сообщение и место → `tracing::error!(target: "panic")` (а значит, и в журнал Windows), затем
  прежний хук. `fatal` — ошибка в журнал, `eprintln`, `exit(1)`.
- Экземпляр: именованный mutex `Local\Pets.<app>`; первый создаёт канал `\\.\pipe\Pets.<app>.<сеанс>`
  (`PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE`, сообщения, `PIPE_REJECT_REMOTE_CLIENTS`, один
  экземпляр). Второй подключается, передаёт аргументы (UTF-8 через `\0`, до 64 КиБ) и выходит. Сервер:
  `ConnectNamedPipe` → чтение сообщения целиком (`ERROR_MORE_DATA`) → `DisconnectNamedPipe` → тот же
  описатель снова.

**Тонкости.**

- **Номер сеанса.** `Local\` у mutex — уже в пределах сеанса, а пространство имён каналов общее для всех
  сеансов машины. Без номера (`ProcessIdToSessionId`) второй пользователь RDP или быстрой смены
  подключится к чужому экземпляру (или получит отказ доступа).
- **Первый экземпляр канала.** `FILE_FLAG_FIRST_PIPE_INSTANCE`: если имя уже занял посторонний процесс,
  создание падает, а не открывает ещё один экземпляр чужого канала. Описатель не закрывать между
  клиентами — `DisconnectNamedPipe` и снова `ConnectNamedPipe`: закрытый на миг канал может занять другой.
  Клиент открывает канал с `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION` — сервер не выдаст себя за него.
- **Гонка запуска.** Второй может прийти, когда первый уже взял mutex, но ещё не создал канал: клиент
  повторяет `CreateFileW` (`ERROR_FILE_NOT_FOUND` — пауза 50 мс, `ERROR_PIPE_BUSY` — `WaitNamedPipeW`)
  до 3 с, затем ошибка. Канал создаётся сразу в `single_instance`, до `listen`: подключение и запись
  клиента до `ConnectNamedPipe` не теряются (`ERROR_PIPE_CONNECTED`/`ERROR_NO_DATA` — успех, данные в
  буфере; проверено тестом `args_sent_before_listen_are_kept`).
- **Передний план.** Окно вперёд может вывести только процесс, у которого есть на это право, — его имеет
  запущенный пользователем второй, а не фоновый первый. Второй до записи узнаёт pid сервера
  (`GetNamedPipeServerProcessId`) и вызывает `AllowSetForegroundWindow(pid)`; первый в `on_args` активирует
  окно. Запуск без аргументов тоже доходит (пустое сообщение) — им окно и выводится вперёд.
- **Относительные пути.** Каталог у первого другой: второй раскрывает пути до `single_instance`.
- **Незарегистрированный источник.** `RegisterEventSourceW(null, app)` работает без записи в реестре
  (её делает только администратор), события пишутся. Просмотр событий показывает «The description for
  Event ID 1000 … cannot be found» и после неё нашу строку (тип записи — Error); уровень
  виден как `Level=2`, без названия. `Get-WinEvent -FilterHashtable @{ProviderName='Postman'}` падает
  («нет поставщика») — искать XPath-фильтром:
  `Get-WinEvent -LogName Application -FilterXPath "*[System[Provider[@Name='Postman']]]" -MaxEvents 3`
  (текст — в `.Properties`, `.Message` пуст) или `Get-EventLog -LogName Application -Source Postman`.
- **Установщик [проверить].** Чтобы описание показывалось, установщик (с правами администратора)
  регистрирует источник: ключ `HKLM\SYSTEM\CurrentControlSet\Services\EventLog\Application\<app>`,
  `EventMessageFile` (REG_EXPAND_SZ) = `%SystemRoot%\Microsoft.NET\Framework64\v4.0.30319\EventLogMessages.dll`
  — в этой DLL сообщение «%1» для любых кодов, поэтому код события везде один — 1000. Деинсталлятор ключ
  удаляет.

**Проверка.** `cargo test -p pets-shell` — кодирование сообщения, формат строки события, два вызова
`single_instance` в одном процессе (второй `CreateMutexW` даёт `ERROR_ALREADY_EXISTS` — как у второго
процесса). Вручную — `cargo run -p pets-shell --example probe -- error|panic|fatal|second [арг…]`
(источник `PetsProbe`); `second` в двух окнах терминала — передача между процессами.

**Откуда.** Pets S2-06, крейт `crates/pets-shell`; первый потребитель — Postman.
