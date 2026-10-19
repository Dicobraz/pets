# pets-gpui-windows

Windows-платформа gpui с нашими патчами. Пакет называется как оригинал — `gpui-pre-windows` 0.3.9
(снимок `gpui_windows` из zed@93d7456, Apache-2.0), чтобы подменять его через `[patch]`:

```toml
[patch.crates-io]
gpui-pre-windows = { git = "https://github.com/Dicobraz/pets", rev = "<коммит>" }
```

Каталог исключён из рабочей области `pets` (чужой код со своим стилем и `Cargo.lock`). Наши правки помечены
в коде «Postman patch» / «Postman:» (сделаны в Postman, фаза 0):

| Патч | Файлы | Рецепт |
|---|---|---|
| Шейдеры встроены в exe, компилируются при запуске (сборка без fxc.exe, без путей машины сборки) | `build.rs`, `directx_renderer.rs` | — |
| Кадры по запросу: поток vsync спит без запросов кадра | `vsync.rs`, `platform.rs`, `window.rs`, `events.rs` | `docs/recipes/frames-on-demand.md` |
| Живой ресайз: кадр на `WM_SIZE` в цикле перетаскивания, swapchain окна с растяжением, DirectComposition выключена по умолчанию (`GPUI_DISABLE_DIRECT_COMPOSITION=0` — включить) | `events.rs`, `window.rs`, `directx_renderer.rs`, `platform.rs` | `docs/recipes/live-resize.md` |

Тесты нашей части: `cargo test --manifest-path crates/pets-gpui-windows/Cargo.toml --lib --features test-support vsync`.

При обновлении gpui-kit: взять новый `gpui-pre-windows` с crates.io, перенести помеченные правки, прогнать тесты,
бенч и `resize-check` приложения. Кадры по запросу и живой ресайз — кандидаты в zed-industries/zed.
