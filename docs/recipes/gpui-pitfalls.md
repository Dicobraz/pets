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

## Цена кадра
- Сборка дерева в `render` дёшева (у Урфина ~0,25 мс), дорого то, что gpui делает потом: профиль панели на
  ~70 видимых строк — половина кадра в раскладке taffy (`compute_layout`, flexbox), ещё ~11 % `request_layout`,
  ~9 % paint. Каждая строка из 4–5 flex-`div` с текстом (текст меряется taffy, иногда не раз) — дорого.
  Длинные однородные списки рисовать **одним своим `Element`**: фон — `paint_quad`, текст —
  `text_system().shape_line` + `paint` (кэш строк gpui), клики — по координате на контейнере.
- gpui по умолчанию ограничивает **неактивное** окно 30 кадр/с (`WindowOptions::inactive_frame_interval` =
  33 мс): владелец переключился в другое окно — замер «вдруг» 34 мс на кадр. Бенч: `inactive_frame_interval:
  None`, `focus: false`, `display_id` — неосновной экран, чтобы не мешать владельцу.
- Кэш поддерева: дочерняя view `entity.cached(StyleRefinement::default().size_full())` — кадр без `notify`
  этой view повторяется без раскладки и paint. `notify` дочерней view изнутри `render` родителя действует
  только со следующего кадра (`draw_phase` ≠ None): изменилась — в этом кадре вставить её без `cached`.
- Кэш **не повторяет** `WindowControlArea` (`window_control_hitboxes` нет в `reuse_paint`/`reuse_prepaint`
  gpui-pre 0.3.9): закэшированный заголовок теряет перетаскивание, «─ □ ✕» и Snap. Кэшировать только
  содержимое заголовка, кнопки и зону `Drag` — вне кэшируемой view.
- `GetThreadTimes` квантован тиком планировщика (~15,6 мс): CPU на кадр так не измерить, только за фазу.
- Профиль без WPT: сэмплер в процессе — `SuspendThread` + `GetThreadContext` + `RtlVirtualUnwind`, имена —
  dbghelp `SymFromAddrW` (путь поиска — папка exe, иначе PDB своего exe не найдётся). `CONTEXT` выровнять по
  16 байт (`#[repr(align(16))]`), иначе `ERROR_NOACCESS` (998). Пока поток стоит — не выделять память (куча
  может быть заперта им). Профиль release с `debug = "line-tables-only"`, `strip = false` даёт имена функций.

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

**Откуда.** Урфин, переезд на gpui (ADR-0018, G-01…G-07); Postman (S2), 2026-10-19…20.
- В Rust 1.99 `gpui::Window::window_handle()` (свой метод) затеняет `raw_window_handle::HasWindowHandle` —
  звать явно `HasWindowHandle::window_handle(window)`.
- `PrintWindow` своего окна — не с UI-потока (там он реентерабельно зовёт wndproc gpui посреди render):
  `cx.background_spawn` с HWND как `isize`, ждать в `cx.spawn`. С выключенной DirectComposition
  `PW_RENDERFULLCONTENT` снимает содержимое (Postman S2-05, `--screens`).
- `border` — один цвет на все стороны: цветной маркер слева — отдельным абсолютным дочерним `div`
  (тосты `pets-ui`).
- Fluent: `bundle.set_use_isolating(false)`, иначе U+2068/U+2069 рисуются «тофу» (`pets-i18n`).

- Журнал событий с незарегистрированным источником (без админа): `Get-WinEvent -FilterHashtable
  @{ProviderName=…}` падает «нет провайдера» — `-FilterXPath "*[System[Provider[@Name='X']]]"`, текст —
  в `.Properties`, не `.Message` (рецепт pets-shell).