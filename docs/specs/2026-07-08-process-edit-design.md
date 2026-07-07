# Process Düzenleme (Edit) Özelliği — Tasarım Dokümanı

## Amaç

Guardian uygulamasında mevcut süreç yönetimine (ekleme/silme) düzenleme (edit) özelliği eklemek. Kullanıcılar var olan bir sürecin konfigürasyonunu (`id` hariç tüm alanlar) değiştirebilecek.

## Kapsam

- **Rust Backend**: `update_process` Tauri command'i + `process_manager.rs`'e yeni metod
- **İzinler**: `commands.toml`'a `update_process` ekleme
- **Vue Frontend**: Mevcut ekleme modal'ını Ekle/Düzenle ortak kullanacak şekilde genişletme
- **Test**: Rust unit testi

## Detaylı Tasarım

### 1. Rust — `process_manager.rs`

```rust
pub fn update_process(&mut self, id: &str, config: ProcessConfig) -> Result<(), String>
```

**Davranış:**
- `config.id` yoksayılır (`id` parametresi sabit kalır)
- SQLite: `UPDATE processes SET name=?, command=?, args=?, cwd=?, auto_restart=?, max_restarts=?, auto_start=? WHERE id=?`
- Runtime: `self.processes[id].config` alanlarını güncelle (status, pid, start_time vs. korunur)
- Eğer process `Running` durumundaysa: config güncellenir ama process yeniden başlatılmaz (kullanıcı isterse manuel restart eder)
- Eğer `id` bulunamazsa: `Err("Process not found")`

**Sınırlar:**
- ID değiştirilemez (primary key)
- Çalışan process'in config'i güncellenir ama spawn edilmez

### 2. Rust — `lib.rs`

```rust
#[tauri::command]
async fn update_process(
    state: tauri::State<'_, Arc<Mutex<ProcessManager>>>,
    id: String,
    config: ProcessConfig,
) -> Result<(), String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    manager.update_process(&id, config)
}
```

`invoke_handler`'a kaydedilecek: `.invoke("update_process", ...)`.

### 3. İzinler — `commands.toml`

```toml
commands.allow = [
  "get_processes", "start_process", "stop_process",
  "add_process", "remove_process", "get_process_logs",
  "update_process"
]
```

### 4. Vue Frontend — `App.vue`

**State değişiklikleri:**
- `isAddingProcess` → `isProcessModalOpen: boolean` (modal göster/gizle)
- `editingProcessId: string | null` (null = ekleme modu, dolu = düzenleme modu)
- `newProcess` → modal'ın form model'i olarak kalır, düzenleme modunda doldurulur

**Yeni buton — her process kartında:**
```
[▶ Başlat] [■ Durdur] [📋 Loglar] [✏️ Düzenle] [🗑️ Sil]
```

**`editProcess(id)` metodu:**
1. `processes` listesinden ilgili process'i bul
2. `newProcess`'i process verileriyle doldur (id, name, command, args, cwd, vb.)
3. `editingProcessId = id` ata
4. `isProcessModalOpen = true` yap

**Modal davranışı:**
- `editingProcessId === null` → "Yeni Süreç Ekle" başlığı, `id` input'u düzenlenebilir
- `editingProcessId !== null` → "Süreci Düzenle: {name}" başlığı, `id` input'u `disabled` (readonly), diğer alanlar dolu gelir

**Kaydet butonu:**
- Ekleme modu: `invoke("add_process", { config })`
- Düzenleme modu: `invoke("update_process", { id: editingProcessId, config })` — `config.id` yoksayılır
- Başarılı: modal kapanır, process listesi yenilenir
- Hata: Swal hata mesajı gösterilir

**Modal kapatma / reset:**
```ts
closeModal() {
  isProcessModalOpen = false;
  editingProcessId = null;
  resetForm();
}
```

### 5. Test — `process_manager.rs`

`#[cfg(test)]` içinde yeni test:
- `test_update_process`: config oluştur, ekle, güncelle, doğrula
- `test_update_process_not_found`: var olmayan ID ile güncelleme → hata döndürmeli

## Kapsam Dışı

- Çalışan process'in otomatik restart'ı (kullanıcı manuel restart eder)
- Batch düzenleme
- Geçmiş/audit log'u
