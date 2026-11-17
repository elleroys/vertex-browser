# Vortex Browser (Windows 11 — Rust + Tauri 2 + WebView2 + SQLite)

**Vortex Browser** — це незалежний браузер для Windows 11, орієнтований на мінімальне споживання RAM, високу швидкодію, приватність та зручність роботи з великою кількістю вкладок.

## Архітектура проєкту

- `src-tauri/` — Backend на Rust (Tauri 2):
  - `src/webview_manager.rs` — керування кількома незалежними дочірніми екземплярами **Microsoft WebView2** (`tauri::webview::WebviewBuilder` + `ICoreWebView2`) всередині єдиного вікна без використання `iframe`.
  - `src/adblock.rs` — двигун `adblock-rust` з підтримкою **EasyList** та **EasyPrivacy**, перехопленням мережевих запитів на рівні `ICoreWebView2::WebResourceRequested`, автооновленням списків та домено-специфічним білим списком (Whitelist).
  - `src/password_manager.rs` — локальне сховище паролів із виведенням ключа через **Argon2id** (`m=65536 KiB, t=3, p=1`), шифруванням **XChaCha20-Poly1305**, затиранням ключів у пам'яті (`zeroize`) та імпортом CSV із Google Password Manager.
  - `src/memory_saver.rs` — призупинення (`ICoreWebView2_13::TrySuspend`) та вивантаження (`Tab Discarding`) неактивних вкладок із захистом вкладок зі звуком, активних завантажень і закріплених вкладок.
  - `src/downloads.rs` — потоковий менеджер завантажень з підтримкою HTTP `Range` (пауза/відновлення).
  - `src/db.rs` — локальна БД **SQLite** (`rusqlite`, режим `WAL`) для сесії вкладок, закладок, історії, завантажень, винятків AdBlock та налаштувань.
  - `capabilities/default.json` — суворий ACL Tauri 2: доступ до IPC-команд має виключно головне вікно (`main`), зовнішні сайти у `tab-*` повністю ізольовані.
- `src/` — інтерфейс браузера на React + TypeScript + Tailwind CSS (українська мова, темна тема).
- `server.ts` — локальний HTTP-сервер та проксі з SQLite (`node:sqlite`) для тестування та роботи інтерфейсу поза середовищем Windows WebView2.
- `benchmarks/compare_vortex_vs_chrome.ps1` — PowerShell-скрипт для порівняльного вимірювання Vortex vs Chrome на Windows 11 (1, 5, 10, 20, 30 вкладок).

## Збірка інсталятора для Windows 11

### Передумови
1. Windows 11 (x64) з установленим **Microsoft Edge WebView2 Runtime** (вбудовано у Windows 11).
2. **Rust** (`rustup` stable-x86_64-pc-windows-msvc) + **Visual Studio Build Tools 2022** (Desktop development with C++).
3. **Node.js 20+**.

### Команди розробки та збірки

```bash
# 1. Встановлення залежностей frontend та Tauri CLI
npm install
npm install -D @tauri-apps/cli

# 2. Перевірка компіляції Rust backend
cd src-tauri && cargo check && cd ..

# 3. Запуск у режимі розробки (Windows 11 + WebView2)
npx tauri dev

# 4. Збірка релізного інсталятора (.exe NSIS та .msi) для Windows 11
npx tauri build
```

Готові інсталятори будуть створені у:
- `src-tauri/target/release/bundle/nsis/Vortex Browser_1.0.0_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Vortex Browser_1.0.0_x64_en-US.msi`

## Порівняльне тестування Vortex vs Chrome

```powershell
powershell -ExecutionPolicy Bypass -File .\benchmarks\compare_vortex_vs_chrome.ps1
```

Отриманий файл `benchmarks/vortex_vs_chrome_report.json` можна безпосередньо імпортувати на сторінці `vortex://benchmarks` у браузері.
