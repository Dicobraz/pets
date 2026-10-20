# Грабли gpui и стенда

**Применимость:** любое приложение на gpui (gpui-kit 0.7.3 = снимок `gpui-pre` 0.3.9), Windows.

Короткие находки: что неожиданно и как обойти. Дополнять по ходу работы.

## Ввод
- `gpui_kit::open_window` оборачивает view в свой `Root`: он вешает Tab/Shift+Tab на переход фокуса и Ctrl+C
  на Copy (контекст `Root`). Привязки срабатывают **раньше** `on_key_down`/`capture_key_down` — Tab туда не
  доходит. Свой конвейер клавиш — через `cx.intercept_keystrokes` (до привязок; `cx.stop_propagation()`, если
  клавиша забрана), или свои `key_context` + `cx.bind_keys` (глубже `Root` — главнее).
- Windows-платформа gpui сначала шлёт окну своё `WM_GPUI_KEYDOWN` (`WM_USER+8`, приватная константа) с теми же
  параметрами и только если gpui клавишу не обработал — настоящий `WM_KEYDOWN`. Хук окна (subclass), которому
  нужны все нажатия, ловит `WM_GPUI_KEYDOWN` и пропускает следующий за ним такой же `WM_KEYDOWN`. При
  обновлении gpui константу перепроверить.
- Shift+цифра и Shift+знак приходят готовым символом без Shift (`Shift+1` → `"!"`), серый «+» и `Shift+=` —
  оба `"+"`: различать по коду клавиши из хука. Ctrl+серая `*` не даёт ни клавиши, ни символа.

## Окно
- `observe_window_bounds` срабатывает на ресайз, но не на перемещение окна: геометрию для сессии снимать ещё и
  в `on_window_should_close`. Координаты `window_bounds()` — свои логические, но круговой путь (сохранить →
  открыть с этими `WindowBounds`) точный.
- Скриншоты `computer-use` снимают один монитор: окно на втором не видно — `switch_display` или
  `GetWindowRect` + `CopyFromScreen`/`PrintWindow`.

## Простой и кадры
- Без `pets-gpui-windows` gpui в простое ~2 % ядра (поток vsync); с патчем — 0,05 %.
- Будить UI из воркеров: каналов gpui не экспортирует — `futures::channel::mpsc::unbounded` + задача
  `cx.spawn` на основном потоке, пачку сигналов сливать в один проход, `wake_after` — `background_executor().timer`.
- Таймер на каждом кадре (`spawn` + `timer` + `notify` в `render`) — цепочка самопробуждений: держать один
  отложенный таймер (флаг «уже стоит»).

## Рендер
- gpui DX11 перебирает адаптеры `EnumAdapters` и берёт первый, где создаётся устройство; WARP («Microsoft
  Basic Render Driver») — последний, так что при сбое оборудования откат на WARP автоматический.
  Принудительно — `GPUI_FORCE_WARP=1` (патч `pets-gpui-windows`); в Rust 2024 `std::env::set_var` — `unsafe`,
  ставить до запуска потоков.

## Сборка и зависимости
- Не делать `cargo update -p <патченный крейт>`: MSRV-резолвер переразрешает чужие зависимости (у Урфина
  `gpu-allocator` уехал на `windows 0.58` — сломался `wgpu-hal`). Вернуть `Cargo.lock` и дать cargo применить
  патч самому (`cargo metadata`).
- cargo-deny с gpui-kit: лицензии 0BSD (`enum-iterator*`) и CC0-1.0 (`hexf-parse`, `notify`, `tiny-keccak`) —
  исключениями по крейтам; «unmaintained» (instant, paste, rustybuzz, ttf-parser, zbus/zvariant,
  rustls-pemfile) — в `ignore` с причиной; `reqwest` у gpui-kit-assets — только wasm.
- Вес: приложение только на gpui — 20–24 МБ; egui и gpui в одном exe — ~30 МБ.
- Несколько тяжёлых сборок параллельно (агенты, release с LTO) роняют rustc: `STATUS_STACK_BUFFER_OVERRUN`
  (0xc0000409). Лечится повтором с `-j 4`; замеры — только на тихой машине.

## Стенд владельца
- Два монитора.
- В Windows выключены «Эффекты прозрачности» (Mica не видна ни у одного окна) и «Макеты прикрепления при
  наведении на кнопку развёртывания» — перед погоней за «багом» проверить реестр (`Themes\Personalize`,
  `Explorer\Advanced`).

**Откуда.** Урфин, переезд на gpui (ADR-0018, G-01…G-07), 2026-10-19…20.
