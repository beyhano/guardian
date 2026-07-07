# Guardian - Developer Guide (CLAUDE.md)

This file outlines build, test, and development instructions, along with codebase conventions for the Guardian Windows Supervisor application.

## Build and Run Commands

* **Dev Environment:** `pnpm tauri dev` (runs the Vite frontend and compiles/launches the Tauri desktop app)
* **Production Build:** `pnpm tauri build`
* **Rust Unit Tests:** `cargo test --manifest-path src-tauri/Cargo.toml`
* **Frontend TypeScript/Vue Check:** `pnpm build`

## Architecture & Code Conventions

### Rust Backend (`src-tauri/src/`)
* **State Management:** Core process monitoring engine resides in [`process_manager.rs`](file:///c:/Users/beyhan/Desktop/Projeler/Rust/guardian/src-tauri/src/process_manager.rs) using `tokio::process` for async execution and standard thread-safe locks (`Arc<Mutex<HashMap>>`).
* **Avoid Async Recursion Cycles:** If an async function spawns a task that calls another async function in a mutual recursive loop (e.g. `spawn_process_internal` and `handle_process_exit`), break the type-checking layout loop by using boxed futures via `std::pin::Pin<Box<dyn std::future::Future + Send>>` (e.g., `spawn_process_boxed`).
* **Windows Process Trees:** When terminating processes on Windows, use `taskkill /F /T /PID <pid>` to force-kill the entire process tree, avoiding orphaned child processes.
* **Console Windows Prevention:** Always set `CREATE_NO_WINDOW` (`0x08000000`) creation flags on Windows to prevent flash console frames.
* **Process Auto-Resume on Startup:** Spawn processes marked with `auto_start` during `setup` in [`lib.rs`](file:///c:/Users/beyhan/Desktop/Projeler/Rust/guardian/src-tauri/src/lib.rs) using `auto_start_processes` to automatically restore execution states upon application startup.

### Tauri v2 IPC & Security (`src-tauri/capabilities/` & `src-tauri/permissions/`)
* **Permissions Configuration:** All custom local backend commands must be declared in [`src-tauri/permissions/commands.toml`](file:///c:/Users/beyhan/Desktop/Projeler/Rust/guardian/src-tauri/permissions/commands.toml) under the `allow-all-commands` identifier.
* **Underscores Constraint:** Do not add raw permission identifiers containing underscores directly to [`capabilities/default.json`](file:///c:/Users/beyhan/Desktop/Projeler/Rust/guardian/src-tauri/capabilities/default.json). Instead, group commands in a TOML permission file with hyphenated identifiers and reference them using `app:<permission-identifier>` (e.g., `"app:allow-all-commands"`).

### Vue Frontend (`src/`)
* **Framework:** Vue 3 Composition API using `<script setup lang="ts">`.
* **State Sync:** Listen to real-time events emitted by Rust:
  * `"log-line"`: Receives new output strings from stdout/stderr.
  * `"status-changed"`: Informs when process state switches.
* **Styling:** Premium dark glassmorphic design system defined using custom CSS variables in [`App.vue`](file:///c:/Users/beyhan/Desktop/Projeler/Rust/guardian/src/App.vue).

## Zorunlu Kontroller (Mandatory Checks)

1.  **Dokümantasyon (Documentation)**:
    *   **Kural**: `docs/wiki/` klasöründeki Obsidian markdown belgeleri güncellenmelidir. Yeni tablo, kavram veya endpoint eklendiğinde `docs/wiki/Index.md` de güncellenmelidir. Obsidian bağlantıları `[[Link]]` formatında kalmalıdır.
    *   **Yapay Zeka Yorumu**: Yeni bir özellik, model, servis veya endpoint eklendiğinde, ilgili **Obsidian markdown belgeleri** (`docs/wiki/` altında) otomatik olarak güncellenmelidir. Özellikle `docs/wiki/Index.md` yeni eklemelerin referansını içermelidir. Bağlantılar `[[Link]]` syntax'ına uygun olmalıdır.

2.  **Güvenlik (Unsafe Rust)**:
    *   **Kural**: Rust `unsafe` blokları KESİNLİKLE KULLANILMAMALIDIR. FFI çağrıları, ham pointer işlemleri, transmute, union erişimi gibi işlemler yasaktır. Safe Rust ile çözülemeyen bir durum varsa alternatif yaklaşım önerilmelidir.
    *   **Yapay Zeka Yorumu**: Kod üretiminde veya düzenlemesinde, **hiçbir koşulda** `unsafe` anahtar kelimesiyle başlayan Rust blokları veya FFI gibi güvenli olmayan mekanizmalar kullanılmamalıdır. Eğer bir görev `unsafe` gerektiriyorsa, durmalı ve safe Rust yaklaşımları veya alternatif tasarımlar önermelidir. Bu, projenin temel güvenlik prensibidir.

3.  **Test & Profiling**:
    *   **Kural**: Her değişiklikten sonra `cargo check` (derleme hatası olmamalı), `cargo test` (mevcut testler geçmeli) ve performans kritik eklemelerde `cargo bench` veya el ile profil kontrolü yapılmalıdır.
    *   **Yapay Zeka Yorumu**: Her kod değişikliğinden **sonra**, şu komutlar **mutlaka** çalıştırılmalı ve başarılı olmalıdır:
        *   `cargo check` (Derleme başarısız olursa, kod düzeltilmeli.)
        *   `cargo test` (Tüm testler geçerli olmalı.)
    *   Eğer yapılan ekleme performans açısından kritikse, `cargo bench` komutu ile performans testleri de yapılmalıdır. Bu adımlar tamamlanmadan bir görev "bitmiş" sayılmaz.


## Dil (Language)

*   **Kod, yorum, commit mesajları**: **Türkçe** olmalıdır.
*   **Teknik terimler**: `endpoint`, `model`, `async`, `trait` gibi teknik terimler İngilizce olmalidir.
*   **Yapay Zeka Yorumu**: Kod yazarken, yorum yaparken ve commit mesajları oluştururken **Türkçe** kullanılmalıdır. Teknik terimler orijinal İngilizce hallerinde kalmalidir.

Bu kurallar, projede tutarlılık, güvenlik ve kalite sağlamak için zorunludur. Yapay zeka olarak, bu belgedeki her kuralı katı bir şekilde uygulamanız gerekmektedir.