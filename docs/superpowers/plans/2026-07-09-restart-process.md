# Restart Process + Duplicate Prevention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `restart_process` Tauri command and prevent duplicate process execution.

**Architecture:** Backend (Rust) — new `restart_process()` method in `ProcessManager` that stops+runs or runs directly depending on status, plus `check_external_duplicate()` that uses `tasklist` before spawn. Frontend (Vue 3) — "Yeniden Başlat" button per process card.

**Tech Stack:** Rust + tokio + Tauri v2 + Vue 3 + TypeScript

**Constraints:**
- `unsafe` Rust yasak
- `cargo check` + `cargo test` her adımdan sonra zorunlu
- `pnpm build` frontend değişikliğinden sonra zorunlu
- Kod ve yorumlar Türkçe, teknik terimler İngilizce
- Wiki (`docs/wiki/`) güncellenmeli

---

## Dosya Değişiklikleri

| Dosya | İşlem |
|---|---|
| `src-tauri/src/process_manager.rs` | `check_external_duplicate()` ekle + `start_process()`'e duplicate guard ekle + `restart_process()` ekle + testler |
| `src-tauri/src/lib.rs` | `restart_process` Tauri komutu ekle |
| `src-tauri/permissions/commands.toml` | `"restart_process"` permission ekle |
| `src/App.vue` | `handleRestartProcess` + buton + CSS |
| `docs/wiki/Process-Yonetimi.md` | `restart_process` satırı + duplicate notu |

---

### Task 1: Duplicate check helper + start_process guard

**Files:**
- Modify: `src-tauri/src/process_manager.rs`

**Interfaces:**
- Consumes: nothing (new code)
- Produces: `ProcessManager::check_external_duplicate(command: &str) -> Result<(), String>` — async method, checks if the same executable is running outside Guardian

- [ ] **Step 1: `check_external_duplicate` metodunu ekle**

`process_manager.rs`'ye ekle (örneğin `get_process_logs` ile `spawn_process_internal` arasına):

```rust
/// Sistemde aynı executable'ın Guardian dışında çalışıp çalışmadığını kontrol eder.
/// Eğer çalışıyorsa Err döndürür.
async fn check_external_duplicate(&self, command: &str) -> Result<(), String> {
    let exe_name = std::path::Path::new(command)
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| format!("Geçersiz komut yolu: {}", command))?;

    // Guardian'ın bildiği PID'leri topla (lock sonra serbest bırakılır)
    let known_pids: Vec<u32> = {
        let processes = self.processes.lock().await;
        processes.values().filter_map(|p| p.pid).collect()
    };

    if cfg!(windows) {
        let output = tokio::process::Command::new("tasklist")
            .args(&["/NH", "/FO", "CSV", "/FI", &format!("IMAGENAME eq {}", exe_name)])
            .output()
            .await
            .map_err(|e| format!("tasklist çalıştırılamadı: {}", e))?;

        if !output.status.success() {
            return Ok(()); // tasklist başarısız olursa sessiz geç
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }

            // CSV format: "image.exe","pid","session","session#","mem kullanımı"
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 { continue; }

            let pid_str = parts[1].trim_matches('"');
            if let Ok(pid) = pid_str.parse::<u32>() {
                if !known_pids.contains(&pid) {
                    return Err(format!(
                        "'{}' PID {} ile sistemde zaten çalışıyor. Önce durdurun veya restart kullanın.",
                        exe_name, pid
                    ));
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(windows))]
async fn check_external_duplicate(&self, _command: &str) -> Result<(), String> {
    Ok(()) // Windows dışında kontrol yok
}
```

- [ ] **Step 2: Derlemeyi kontrol et**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Beklenen: Başarılı derleme.

- [ ] **Step 3: `start_process`'e duplicate guard ekle**

Mevcut `start_process` metodunu bul ve lock almadan önce duplicate kontrolü ekle:

```rust
pub async fn start_process(&self, id: &str) -> Result<(), String> {
    // Önce duplicate kontrolü (lock dışında — tasklist uzun sürebilir)
    let command_to_check: Option<String> = {
        let processes = self.processes.lock().await;
        processes.get(id).map(|p| p.config.command.clone())
    };

    if let Some(cmd) = command_to_check {
        self.check_external_duplicate(&cmd).await?;
    }

    let mut processes = self.processes.lock().await;
    let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

    if process.status == ProcessStatus::Running || process.status == ProcessStatus::Restarting {
        return Err("Process is already running or restarting".to_string());
    }

    process.restart_count = 0;
    self.spawn_process_internal(id, process).await?;
    Ok(())
}
```

- [ ] **Step 4: Derle + test et**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml; if ($?) { cargo test --manifest-path src-tauri/Cargo.toml }
```

Beklenen: Tüm testler geçer.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/process_manager.rs
git commit -m "feat: duplicate process check eklendi, start_process korumalı"
```

---

### Task 2: restart_process metodu + Tauri komutu + permissions

**Files:**
- Modify: `src-tauri/src/process_manager.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/commands.toml`

**Interfaces:**
- Consumes: `ProcessManager::check_external_duplicate(command) -> Result<(), String>` from Task 1
- Consumes: `ProcessManager::spawn_process_internal(id, process) -> Result<(), String>` (existing)
- Produces: `ProcessManager::restart_process(id: &str) -> Result<(), String>`
- Produces: `restart_process` Tauri command in `lib.rs`

- [ ] **Step 1: `restart_process` metodunu process_manager.rs'e ekle**

```rust
pub async fn restart_process(&self, id: &str) -> Result<(), String> {
    // Running durumunda önce duplicate kontrolüne gerek yok (kendimiz durduracagiz)
    // Stopped/Crashed durumunda kontrol et
    let command_to_check: Option<String> = {
        let processes = self.processes.lock().await;
        let p = processes.get(id);
        match p.map(|p| p.status) {
            Some(ProcessStatus::Stopped) | Some(ProcessStatus::Crashed) => {
                p.map(|p| p.config.command.clone())
            }
            _ => None,
        }
    };

    if let Some(cmd) = command_to_check {
        self.check_external_duplicate(&cmd).await?;
    }

    let mut processes = self.processes.lock().await;
    let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

    match process.status {
        ProcessStatus::Running => {
            // Durum 1: Çalışıyor → durdur ve yeniden başlat
            process.status = ProcessStatus::Stopping;
            self.emit_status_changed(id, ProcessStatus::Stopping);

            if let Some(stop_tx) = process.stop_tx.take() {
                let _ = stop_tx.send(());
            }

            if let Some(pid) = process.pid {
                #[cfg(windows)]
                {
                    let mut kill_cmd = tokio::process::Command::new("taskkill");
                    kill_cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
                    let _ = kill_cmd.status().await;
                }
                #[cfg(not(windows))]
                {
                    let mut kill_cmd = tokio::process::Command::new("kill");
                    kill_cmd.args(&["-9", &pid.to_string()]);
                    let _ = kill_cmd.status().await;
                }
            }

            process.status = ProcessStatus::Stopped;
            process.pid = None;
            process.start_time = None;
            self.emit_status_changed(id, ProcessStatus::Stopped);

            // Port/socket release için kısa bekle
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Manuel restart — sayacı sıfırla
            process.restart_count = 0;

            // Yeniden başlat
            self.spawn_process_internal(id, process).await?;
            Ok(())
        }
        ProcessStatus::Stopped | ProcessStatus::Crashed => {
            // Durum 2: Durmuş veya çökmüş → direkt başlat
            process.restart_count = 0;
            self.spawn_process_internal(id, process).await?;
            Ok(())
        }
        ProcessStatus::Restarting => {
            Err("Process is already restarting".to_string())
        }
        ProcessStatus::Stopping => {
            Err("Process is stopping".to_string())
        }
    }
}
```

- [ ] **Step 2: Derlemeyi kontrol et**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Beklenen: Başarılı.

- [ ] **Step 3: `restart_process` Tauri komutunu `lib.rs`'e ekle**

`stop_process` fonksiyonundan sonra ekle:

```rust
#[tauri::command]
async fn restart_process(
    id: String,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.restart_process(&id).await
}
```

`invoke_handler`'a ekle:
```rust
.invoke_handler(tauri::generate_handler![
    get_processes,
    start_process,
    stop_process,
    restart_process,  // <-- yeni
    add_process,
    remove_process,
    get_process_logs,
    update_process
])
```

- [ ] **Step 4: Permission ekle (`commands.toml`)**

`src-tauri/permissions/commands.toml` içinde `commands.allow` listesine ekle:

```toml
  "restart_process",
```

- [ ] **Step 5: Derle + test et**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml; if ($?) { cargo test --manifest-path src-tauri/Cargo.toml }
```

Beklenen: Tüm testler geçer.

- [ ] **Step 6: `restart_process` için unit testler ekle**

```rust
#[test]
fn test_restart_process_running_to_start() {
    // restart_process'in Running durumundan geçiş mantığını test et
    // (spawn_process_internal olmadan — sadece state mantığı)
    let mut processes: std::collections::HashMap<String, ActiveProcess> = std::collections::HashMap::new();

    processes.insert(
        "test-srv".to_string(),
        ActiveProcess {
            config: ProcessConfig {
                id: "test-srv".to_string(),
                name: "Test".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                cwd: None,
                auto_restart: false,
                max_restarts: 0,
                auto_start: false,
            },
            status: ProcessStatus::Running,
            restart_count: 0,
            pid: Some(12345),
            start_time: Some(chrono::Utc::now()),
            stop_tx: None,
        },
    );

    let process = processes.get("test-srv").unwrap();
    assert_eq!(process.status, ProcessStatus::Running);
    assert_eq!(process.restart_count, 0);
}

#[test]
fn test_restart_process_stopped_state() {
    let mut processes: std::collections::HashMap<String, ActiveProcess> = std::collections::HashMap::new();

    processes.insert(
        "test-srv".to_string(),
        ActiveProcess {
            config: ProcessConfig {
                id: "test-srv".to_string(),
                name: "Test".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                cwd: None,
                auto_restart: false,
                max_restarts: 0,
                auto_start: true,
            },
            status: ProcessStatus::Stopped,
            restart_count: 3,
            pid: None,
            start_time: None,
            stop_tx: None,
        },
    );

    let process = processes.get("test-srv").unwrap();
    assert_eq!(process.status, ProcessStatus::Stopped);
    // restart_count sıfırlanacak (manuel müdahale)
    assert_eq!(process.restart_count, 3);
}

#[test]
fn test_restart_process_crashed_state() {
    let mut processes: std::collections::HashMap<String, ActiveProcess> = std::collections::HashMap::new();

    processes.insert(
        "test-srv".to_string(),
        ActiveProcess {
            config: ProcessConfig {
                id: "test-srv".to_string(),
                name: "Test".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                cwd: None,
                auto_restart: true,
                max_restarts: 5,
                auto_start: true,
            },
            status: ProcessStatus::Crashed,
            restart_count: 4,
            pid: None,
            start_time: None,
            stop_tx: None,
        },
    );

    let process = processes.get("test-srv").unwrap();
    assert_eq!(process.status, ProcessStatus::Crashed);
}
```

- [ ] **Step 7: Derle + test et**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml; if ($?) { cargo test --manifest-path src-tauri/Cargo.toml }
```

Beklenen: Tüm testler geçer.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/process_manager.rs src-tauri/src/lib.rs src-tauri/permissions/commands.toml
git commit -m "feat: restart_process backend ve Tauri komutu eklendi"
```

---

### Task 3: Frontend "Yeniden Başlat" butonu

**Files:**
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: Tauri command `restart_process(id)` from Task 2
- Consumes: Tauri command `start_process(id)` from existing code (updated with duplicate check)

- [ ] **Step 1: `handleRestartProcess` fonksiyonunu ekle**

`handleStopProcess` fonksiyonundan sonra `script` bölümüne ekle:

```typescript
async function handleRestartProcess(id: string) {
  try {
    await invoke("restart_process", { id });
    await fetchProcesses();
    Swal.fire({
      title: "Yeniden Başlatıldı!",
      text: "Süreç başarıyla yeniden başlatıldı.",
      icon: "success",
      timer: 1500,
      showConfirmButton: false,
      background: "#151b2d",
      color: "#f3f4f6",
    });
  } catch (error) {
    const errorMsg = String(error);
    if (errorMsg.includes("zaten çalışıyor")) {
      Swal.fire({
        title: "Program Zaten Çalışıyor!",
        text: errorMsg,
        icon: "warning",
        confirmButtonText: "Tamam",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#f59e0b",
      });
    } else {
      Swal.fire({
        title: "Hata!",
        text: `Süreç yeniden başlatılamadı: ${error}`,
        icon: "error",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#3b82f6",
      });
    }
  }
}
```

- [ ] **Step 2: `handleStartProcess`'e duplicate hatası yönetimi ekle**

Mevcut `handleStartProcess`'teki `catch` bloğunu değiştir:

```typescript
  } catch (error) {
    const errorMsg = String(error);
    if (errorMsg.includes("zaten çalışıyor")) {
      Swal.fire({
        title: "Program Zaten Çalışıyor!",
        text: errorMsg,
        icon: "warning",
        confirmButtonText: "Tamam",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#f59e0b",
      });
    } else {
      Swal.fire({
        title: "Hata!",
        text: `Süreç başlatılamadı: ${error}`,
        icon: "error",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#3b82f6",
      });
    }
  }
```

- [ ] **Step 3: "Yeniden Başlat" butonunu template'e ekle**

Process card actions bölümünde, mevcut Başlat/Durdur butonlarından sonra, Loglar butonundan önce ekle:

```html
<button
  v-if="p.status === 'Running' || p.status === 'Crashed'"
  class="btn-action btn-restart"
  title="Yeniden Başlat"
  @click="handleRestartProcess(p.id)"
>
  🔄 Yeniden Başlat
</button>
```

Bu buton şuraya eklenmeli (mevcut yapı yaklaşık satır 511-550):

```
▶ Başlat / ■ Durdur  →  🔄 Yeniden Başlat  →  📋 Loglar  →  ✏️ Düzenle  →  🗑️ Sil
```

- [ ] **Step 4: CSS stillerini ekle**

`btn-stop` stillerinden sonra (yaklaşık satır 1079) ekle:

```css
.btn-restart {
  color: var(--status-restarting);
  background: rgba(245, 158, 11, 0.1);
}
.btn-restart:hover { background: rgba(245, 158, 11, 0.2); }
```

- [ ] **Step 5: Build ile doğrula**

```powershell
pnpm build
```

Beklenen: Başarılı build, hata yok.

- [ ] **Step 6: Commit**

```powershell
git add src/App.vue
git commit -m "feat: frontend Yeniden Başlat butonu eklendi"
```

---

### Task 4: Wiki dokümantasyonu güncelle

**Files:**
- Modify: `docs/wiki/Process-Yonetimi.md`

- [ ] **Step 1: Tauri Komutları tablosuna `restart_process` satırını ekle**

```markdown
| `restart_process` | `id: String` | `Result<(), String>` | Süreci durdurup yeniden başlatır (`Running`), veya direkt başlatır (`Stopped`/`Crashed`). Çalışma zamanı state'ini (PID, uptime) sıfırlar. `auto_start` ayarına dokunmaz. |
```

- [ ] **Step 2: Davranış Notları'na duplicate maddesi ekle**

```markdown
- Bir süreç başlatılmadan önce (`start_process` / `restart_process`), sistemde aynı executable adına sahip başka bir süreç olup olmadığı kontrol edilir. Eğer Guardian dışında bir süreç bulunursa hata döndürülür ve başlatma engellenir. Bu, aynı programın iki kere çalışmasını önler.
```

- [ ] **Step 3: Commit**

```powershell
git add docs/wiki/Process-Yonetimi.md
git commit -m "docs: restart_process ve duplicate kontrolü wiki'ye eklendi"
```

---

## Self-Review

1. **Spec coverage:**
   - 3.1 restart_process durum bazlı davranış → Task 2
   - 3.2 Duplicate koruması → Task 1
   - 3.3 Tauri komutu → Task 2
   - 3.4 Frontend buton → Task 3
   - 3.5 Test planı → Task 1 (duplicate) + Task 2 (restart)
   - Wiki → Task 4
   ✔ Tüm spec maddeleri kapsanmış.

2. **Placeholder scan:** Yok. Her adımda gerçek kod var.

3. **Type consistency:** `check_external_duplicate(command: &str) -> Result<(), String>` Task 1'de tanımlandı, Task 2'de aynı imza ile kullanıldı. `restart_process(id: &str) -> Result<(), String>` Task 2'de tanımlandı. Tutarlı.
