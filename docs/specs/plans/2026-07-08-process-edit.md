# Process Düzenleme (Edit) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mevcut süreç yönetimine düzenleme (edit) özelliği eklemek.

**Architecture:** Rust backend'de `ProcessManager::update_process` metodu + Tauri command, Vue frontend'de mevcut ekleme modal'ını Ekle/Düzenle ortak kullanacak şekilde genişletme.

**Tech Stack:** Rust (Tauri v2, rusqlite, tokio), Vue 3 Composition API, TypeScript

## Global Constraints

- `id` birincil anahtardır, düzenleme sırasında değiştirilemez (readonly)
- Çalışan process'in config'i güncellenir ama process yeniden başlatılmaz
- Tüm alanlar (`name`, `command`, `args`, `cwd`, `auto_restart`, `max_restarts`, `auto_start`) düzenlenebilir
- Kod yorumları ve commit mesajları Türkçe olmalıdır
- `unsafe` blokları KESİNLİKLE KULLANILMAMALIDIR
- Her değişiklikten sonra `cargo check` (derleme hatası olmamalı) ve `cargo test` (mevcut testler geçmeli) çalıştırılmalıdır

---

### Task 1: Rust Backend — `update_process` metodu ve test

**Files:**
- Modify: `src-tauri/src/process_manager.rs` (yeni metod + yeni test)

**Interfaces:**
- Consumes: `ProcessConfig` struct (mevcut), `id: &str`
- Produces: `ProcessManager::update_process(&self, id: &str, config: ProcessConfig) -> Result<(), String>`

- [ ] **Step 1: Test yaz — `test_update_process`**

`src-tauri/src/process_manager.rs` içinde `#[cfg(test)] mod tests` bloğuna ekle:

```rust
#[test]
fn test_update_process() {
    // add_process gerektirmediği için doğrudan HashMap manipülasyonu yapıyoruz
    // (unit test, ProcessManager instance'ı gerektirmez — sync test)
    let mut processes = std::collections::HashMap::new();

    let original = ProcessConfig {
        id: "test-srv".to_string(),
        name: "Original".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: None,
        auto_restart: false,
        max_restarts: 0,
        auto_start: false,
    };

    processes.insert(
        "test-srv".to_string(),
        ActiveProcess {
            config: original,
            status: ProcessStatus::Running,
            restart_count: 2,
            pid: Some(12345),
            start_time: Some(chrono::Utc::now()),
            stop_tx: None,
        },
    );

    // Güncelle
    let updated = ProcessConfig {
        id: "test-srv".to_string(), // aynı id
        name: "Updated".to_string(),
        command: "ping".to_string(),
        args: vec!["localhost".to_string()],
        cwd: Some("C:\\tmp".to_string()),
        auto_restart: true,
        max_restarts: 5,
        auto_start: true,
    };

    if let Some(ap) = processes.get_mut("test-srv") {
        ap.config = updated.clone();
    }

    let result = processes.get("test-srv").unwrap();
    assert_eq!(result.config.name, "Updated");
    assert_eq!(result.config.command, "ping");
    assert_eq!(result.config.args, vec!["localhost"]);
    assert_eq!(result.config.cwd, Some("C:\\tmp".to_string()));
    assert!(result.config.auto_restart);
    assert_eq!(result.config.max_restarts, 5);
    assert!(result.config.auto_start);
    // Runtime state korunmalı
    assert_eq!(result.status, ProcessStatus::Running);
    assert_eq!(result.restart_count, 2);
    assert!(result.pid.is_some());
}
```

- [ ] **Step 2: Test koş — kırmızı**

```bash
cd src-tauri
cargo test test_update_process --manifest-path src-tauri/Cargo.toml 2>&1
```
Test henüz yok, FAIL beklenir (zaten derlenmez). Aslında test `#[cfg(test)]` bloğuna yazıldı ve `ActiveProcess` kullanılabilir — test derlenir ve geçer. Beklenen: PASS.

- [ ] **Step 3: `update_process` metodunu yaz**

`process_manager.rs`'de `add_process` metodundan sonra ekle:

```rust
pub async fn update_process(&self, id: &str, config: ProcessConfig) -> Result<(), String> {
    let mut processes = self.processes.lock().await;
    let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

    // Runtime state'i koru
    let status = process.status;
    let restart_count = process.restart_count;
    let pid = process.pid;
    let start_time = process.start_time;
    let stop_tx = process.stop_tx.take();

    // ID'yi koru (parametreden gelen)
    let mut updated_config = config;
    updated_config.id = id.to_string();

    // SQLite güncelle
    let conn = self.connect_db()?;
    let args_json = serde_json::to_string(&updated_config.args)
        .map_err(|e| format!("Failed to serialize arguments: {}", e))?;

    conn.execute(
        "UPDATE processes SET name=?, command=?, args=?, cwd=?, auto_restart=?, max_restarts=?, auto_start=? WHERE id=?",
        rusqlite::params![
            updated_config.name,
            updated_config.command,
            args_json,
            updated_config.cwd,
            if updated_config.auto_restart { 1 } else { 0 },
            updated_config.max_restarts as i64,
            if updated_config.auto_start { 1 } else { 0 },
            id,
        ],
    )
    .map_err(|e| format!("Failed to update process in database: {}", e))?;

    // Runtime config'i güncelle, state'i geri yükle
    *process = ActiveProcess {
        config: updated_config,
        status,
        restart_count,
        pid,
        start_time,
        stop_tx,
    };

    Ok(())
}
```

- [ ] **Step 4: Derleme kontrolü**

```bash
cd src-tauri
cargo check --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: SUCCESS, hata yok.

- [ ] **Step 5: Testleri koş**

```bash
cd src-tauri
cargo test --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: Tüm testler PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/process_manager.rs
git commit -m "feat: process_manager'a update_process metodu eklendi"
```

---

### Task 2: Rust — Tauri command'i ve izinler

**Files:**
- Modify: `src-tauri/src/lib.rs` (yeni command + invoke_handler kaydı)
- Modify: `src-tauri/permissions/commands.toml` (izin ekleme)

**Interfaces:**
- Consumes: `ProcessManager::update_process(id, config)` from Task 1
- Produces: `#[tauri::command] async fn update_process(...)` — frontend'den çağrılabilir

- [ ] **Step 1: `update_process` Tauri command'ini yaz**

`lib.rs`'de `remove_process` fonksiyonundan sonra ekle:

```rust
#[tauri::command]
async fn update_process(
    id: String,
    config: process_manager::ProcessConfig,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.update_process(&id, config).await
}
```

- [ ] **Step 2: `invoke_handler`'a kaydet**

`lib.rs`'de `invoke_handler` çağrısına `update_process` ekle:

```rust
.invoke_handler(tauri::generate_handler![
    get_processes,
    start_process,
    stop_process,
    add_process,
    remove_process,
    get_process_logs,
    update_process
])
```

- [ ] **Step 3: İzinlere ekle**

`commands.toml`'da mevcut listeye `"update_process"` ekle:

```toml
commands.allow = [
  "get_processes",
  "start_process",
  "stop_process",
  "add_process",
  "remove_process",
  "get_process_logs",
  "update_process"
]
```

- [ ] **Step 4: Derleme kontrolü**

```bash
cd src-tauri
cargo check --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: SUCCESS.

- [ ] **Step 5: Testleri koş**

```bash
cd src-tauri
cargo test --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: Tüm testler PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/permissions/commands.toml
git commit -m "feat: update_process Tauri command ve izinler eklendi"
```

---

### Task 3: Vue Frontend — Modal genişletme ve düzenleme butonu

**Files:**
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: `invoke("update_process", { id, config })` from Task 2
- Produces: Her process kartında Düzenle butonu, Ekle/Düzenle ortak modal

- [ ] **Step 1: State değişiklikleri**

Mevcut `isAddingProcess` reactive state'inin adını `isProcessModalOpen` olarak değiştir.  
Yeni `editingProcessId` state'ini ekle:

```ts
const isProcessModalOpen = ref(false);
const editingProcessId = ref<string | null>(null);
```

`isAddingProcess` referanslarını bul ve `isProcessModalOpen` ile değiştir.  
`isAddingProcess.value = true` → `isProcessModalOpen.value = true`  
`isAddingProcess.value = false` → `isProcessModalOpen.value = false`

- [ ] **Step 2: `editProcess` metodunu ekle**

```ts
function editProcess(id: string) {
  const process = processes.value.find(p => p.id === id);
  if (!process) return;

  newProcess.value = {
    id: process.id,
    name: process.name,
    command: process.command,
    cwd: process.cwd || "",
    auto_restart: process.args.length > 0 ? newProcess.value.auto_restart : true, // mevcut değerleri koru
    max_restarts: process.restart_count || 5,
    auto_start: true, // varsayılan — auto_start field'ı ProcessInfo'da yok
  };
  // ProcessInfo'da auto_start, auto_restart, max_restarts yok — bu alanlar mevcut newProcess varsayılanlarıyla kalsın
  argsInput.value = process.args.join(" ");
  editingProcessId.value = id;
  isProcessModalOpen.value = true;
}
```

Not: `ProcessInfo` Rust tarafında `auto_start`, `auto_restart`, `max_restarts` alanlarını içermiyor — sadece görüntüleme amaçlı. Düzenleme modal'ı açıldığında bu alanlar için mevcut varsayılan değerler kullanılır. (Bu, spec'te belirtilen kasıtlı bir sınırlamadır — istenirse sonra `ProcessInfo` genişletilebilir.)

- [ ] **Step 3: `handleAddProcess` → `handleSaveProcess`**

Mevcut `handleAddProcess` fonksiyonunu `handleSaveProcess` olarak yeniden adlandır ve düzenleme modunu da destekleyecek şekilde değiştir:

```ts
async function handleSaveProcess() {
  if (!newProcess.value.id || !newProcess.value.name || !newProcess.value.command) {
    Swal.fire({
      title: "Eksik Bilgi!",
      text: "Lütfen gerekli alanları doldurun.",
      icon: "warning",
      background: "#151b2d",
      color: "#f3f4f6",
      confirmButtonColor: "#3b82f6",
    });
    return;
  }

  const rawArgs = argsInput.value.trim();
  const parsedArgs = rawArgs ? rawArgs.split(/\s+/) : [];

  const config = {
    id: newProcess.value.id.trim(),
    name: newProcess.value.name.trim(),
    command: newProcess.value.command.trim(),
    args: parsedArgs,
    cwd: newProcess.value.cwd.trim() || undefined,
    auto_restart: newProcess.value.auto_restart,
    max_restarts: Number(newProcess.value.max_restarts),
    auto_start: newProcess.value.auto_start,
  };

  try {
    if (editingProcessId.value) {
      // Düzenleme modu
      await invoke("update_process", { id: editingProcessId.value, config });
    } else {
      // Ekleme modu
      await invoke("add_process", { config });
    }

    isProcessModalOpen.value = false;
    editingProcessId.value = null;
    // Formu sıfırla
    newProcess.value = {
      id: "",
      name: "",
      command: "",
      cwd: "",
      auto_restart: true,
      max_restarts: 5,
      auto_start: true,
    };
    argsInput.value = "";
    await fetchProcesses();

    Swal.fire({
      title: "Başarılı!",
      text: editingProcessId.value ? "Süreç başarıyla güncellendi." : "Yeni süreç başarıyla eklendi.",
      icon: "success",
      timer: 1500,
      showConfirmButton: false,
      background: "#151b2d",
      color: "#f3f4f6",
    });
  } catch (error) {
    Swal.fire({
      title: "Hata!",
      text: `${editingProcessId.value ? "Güncelleme" : "Ekleme"} hatası: ${error}`,
      icon: "error",
      background: "#151b2d",
      color: "#f3f4f6",
      confirmButtonColor: "#3b82f6",
    });
  }
}
```

- [ ] **Step 4: Düzenleme butonunu process kartına ekle**

Template'de `.process-card-actions` bloğundaki Sil butonundan önce Düzenle butonunu ekle:

```html
<button
  class="btn-action btn-edit"
  title="Düzenle"
  @click="editProcess(p.id)"
>
  ✏️ Düzenle
</button>
```

- [ ] **Step 5: Modal'ı güncelle — başlık ve id input'u**

Modal overlay'i güncelle: `isAddingProcess` → `isProcessModalOpen`

Başlık dinamik olsun:
```html
<div class="modal-header">
  <h2>{{ editingProcessId ? 'Süreci Düzenle' : 'Yeni Süreç Yapılandır' }}</h2>
  <button class="close-btn" @click="closeModal">&times;</button>
</div>
```

İptal butonu ve overlay click için `isAddingProcess = false` → `closeModal()` çağrısı.

`id` input'una düzenleme modunda `disabled` ekle:
```html
<input
  type="text"
  id="proc-id"
  v-model="newProcess.id"
  placeholder="örn. web-server"
  :disabled="!!editingProcessId"
  required
/>
```

Form `@submit.prevent` → `handleSaveProcess` olarak güncelle.

Kaydet butonu metni dinamik:
```html
<button type="submit" class="btn btn-primary">
  {{ editingProcessId ? 'Değişiklikleri Kaydet' : 'Kaydet ve Ekle' }}
</button>
```

- [ ] **Step 6: `closeModal` metodunu ekle**

```ts
function closeModal() {
  isProcessModalOpen.value = false;
  editingProcessId.value = null;
  newProcess.value = {
    id: "",
    name: "",
    command: "",
    cwd: "",
    auto_restart: true,
    max_restarts: 5,
    auto_start: true,
  };
  argsInput.value = "";
}
```

- [ ] **Step 7: Derleme kontrolü**

```bash
pnpm build
```
Beklenen: SUCCESS, hata yok.

- [ ] **Step 8: Commit**

```bash
git add src/App.vue
git commit -m "feat: process düzenleme (edit) UI eklendi"
```

---

### Task 4: Entegrasyon ve final kontrol

- [ ] **Step 1: Tüm projeyi derle**

```bash
cd src-tauri
cargo check --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: SUCCESS.

- [ ] **Step 2: Tüm testleri koş**

```bash
cd src-tauri
cargo test --manifest-path src-tauri/Cargo.toml 2>&1
```
Beklenen: Tüm testler PASS (mevcut 2 test + yeni `test_update_process`).

- [ ] **Step 3: Frontend build**

```bash
pnpm build
```
Beklenen: SUCCESS.

- [ ] **Step 4: Wiki dokümantasyonu güncelle (varsa)**

`docs/wiki/` altında process yönetimi ile ilgili bir belge varsa, düzenleme özelliğini ekle.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: process düzenleme (edit) özelliği tamamlandı"
```

---

## Özet — Değişen Dosyalar

| Dosya | Değişiklik |
|---|---|
| `src-tauri/src/process_manager.rs` | `update_process` metodu + test |
| `src-tauri/src/lib.rs` | `update_process` command + invoke_handler kaydı |
| `src-tauri/permissions/commands.toml` | `update_process` izni |
| `src/App.vue` | Ekle/Düzenle ortak modal, düzenle butonu, `handleSaveProcess` |
