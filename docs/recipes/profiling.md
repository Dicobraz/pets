# Профиль без прав администратора

**Применимость:** Windows.

WPR и samply требуют администратора. Без него:

- **Кто будит процесс в простое** — счётчики Windows по потокам: `Win32_PerfRawData_PerfProc_Thread`
  (`ContextSwitchesPersec`, `PercentProcessorTime`) до и после интервала + имена потоков через
  `GetThreadDescription` (доступ `THREAD_QUERY_LIMITED_INFORMATION` = 0x0800).
- **Где тратится CPU** — Very Sleepy (выборка стеков, подключение к процессу по PID, сохранение в файл,
  остановка через N секунд). Установщик Inno Setup распаковывается `innounp` без запуска. Файл `.sleepy` —
  zip с `Symbols.txt` и `Callstacks.txt`, разбирается скриптом.
- **Сборка для профиля** — отдельный каталог `target/prof`: `debug = line-tables-only`,
  `-C link-arg=/OPT:NOICF` (иначе линкер склеивает одинаковые функции и имена в профиле врут),
  `-C force-frame-pointers=yes`.
- **Разбивка кадра** — временные счётчики времени вокруг вызова отрисовки и отправки на GPU.

**Откуда.** Postman S1-02, S1-03.
