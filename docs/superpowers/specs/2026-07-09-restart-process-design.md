# Restart Process + Duplicate Execution Prevention

**Tarih:** 2026-07-09
**İlgili:** [[Process-Yonetimi]], [[Index]]

---

## 1. Amaç

Guardian'da süreçleri **yeniden başlatma** (restart) özelliği yok. Kullanıcı çalışan bir süreci durdurup tekrar başlatmak için iki ayrı işlem yapmak zorunda. Ayrıca `start_process` sistemde aynı programın zaten çalışıp çalışmadığını kontrol etmiyor — bu da aynı programın iki kere başlatılmasına yol açıyor.

## 2. Kapsam

- `ProcessManager::restart_process()` metodu (backend)
- `restart_process` Tauri komutu
- Frontend'de "Yeniden Başlat" butonu
- `start_process` ve `restart_process` için duplicate çalıştırma koruması
- Wiki güncellemesi

## 3. Detaylı Tasarım

### 3.1 `restart_process(id: &str)` — ProcessManager metodu

Durum bazlı davranış:

| Mevcut Durum | Davranış |
|---|---|
| `Running` | `stop_tx` + `taskkill` → 200ms bekle → `spawn_process_internal` |
| `Stopped` | `spawn_process_internal` (tıpkı start gibi) |
| `Crashed` | `spawn_process_internal` (tıpkı start gibi) |
| `Restarting` | Hata: "Process is already restarting" |
| `Stopping` | Hata: "Process is stopping" |

**Önemli davranış farkı (stop'tan farkı):** `restart_process` **DB'de `auto_start`'ı değiştirmez**. Kullanıcı restart istedi — bu geçici bir işlem, auto_start ayarını bozmamalı.

**Running → restart akışı:**
```
status = Stopping
emit status-changed(Stopping)
stop_tx.send(())  // graceful sinyal
taskkill /F /T /PID  // zorla bitir
status = Stopped
emit status-changed(Stopped)
tokio::time::sleep(200ms)  // port/socket release için
restart_count = 0  // restart'ta sayaç sıfırlanır (manuel müdahale)
spawn_process_internal(id, process)  // yeniden başlat
```

### 3.2 Duplicate Çalıştırma Koruması

Yeni bir yardımcı metod: `is_command_running_outside_guardian(command: &str, known_pids: &[u32]) -> bool`

```
command yolundan exe adını çıkar (Path::new(command).file_name())
→ tasklist /NH /FO CSV /FI "IMAGENAME eq exe_adı"
→ Çıktıyı parse et: her satırda "exe_adı","pid","..."
→ Eğer bulunan PID'lerden hiçbiri Guardian'ın bildiği PID'lerde değilse → duplicate var
→ Kullanıcıya hata: "Bu program sistemde zaten çalışıyor (PID: {pid})."
```

**Çağrılacağı yerler:**
- `start_process` içinde, spawn öncesi
- `restart_process` içinde, sadece Stopped/Crashed durumunda spawn öncesi (Running durumunda önce kendi sürecini öldürdüğü için kontrol gerekmez)

### 3.3 Tauri Komutu

```rust
#[tauri::command]
async fn restart_process(
    id: String,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.restart_process(&id).await
}
```

`commands.toml`'a eklenecek:
```toml
"restart_process",
```

`lib.rs` `invoke_handler`'a eklenecek:
```rust
restart_process,
```

### 3.4 Frontend Değişikliği

**Process card butonları — yeni düzen:**

| Süreç Durumu | Buton 1 | Buton 2 | Buton 3 | Buton 4 |
|---|---|---|---|---|
| Running | ■ Durdur | 🔄 Yeniden Başlat | 📋 Loglar | ✏️ Düzenle | 🗑️ Sil |
| Stopped | ▶ Başlat | — | 📋 Loglar | ✏️ Düzenle | 🗑️ Sil |
| Crashed | ▶ Başlat | 🔄 Yeniden Başlat | 📋 Loglar | ✏️ Düzenle | 🗑️ Sil |
| Restarting | (butonlar disabled) | | | | |

"Yeniden Başlat" butonu doğrudan `restart_process` invoke eder, onay sormaz (zaten stop+start yapıyor, iki kere onay gereksiz).

**Duplicate hatası durumunda SweetAlert2 gösterimi:**
```typescript
Swal.fire({
  title: "Program Zaten Çalışıyor!",
  text: `"${exeAdi}" PID ${pid} ile sistemde zaten çalışıyor.`,
  icon: "warning",
  confirmButtonText: "Tamam",
})
```

### 3.5 Test Planı

- `restart_process` unit test: Running → Stopped → Running state geçişleri
- `restart_process` unit test: Stopped durumunda direkt start
- `restart_process` unit test: Restarting durumunda hata döndürme
- Duplicate detection: tasklist çıktısı parse testi

## 4. Kapsam Dışı

- `update_process` sırasında restart tetikleme (ileride eklenebilir)
- Toplu restart (tüm süreçleri yeniden başlat)
- Schedule-based restart (belli aralıklarla restart)

## 5. Riskler

- **Port binding:** Restart sırasında eski süreç portu release etmeden yeni süreç başlamaya çalışabilir. 200ms sleep bunu minimize eder ama garantisi yok. İleride port health check eklenebilir.
- **tasklist parsing:** Windows locale bağlı çıktı formatı. `tasklist /NH /FO CSV` genelde İngilizce/Türkçe'de aynı çalışır ama edge case'ler olabilir.

## 6. Dosya Değişiklik Özeti

| Dosya | İşlem |
|---|---|
| `src-tauri/src/process_manager.rs` | `restart_process()` + `is_command_running_outside_guardian()` ekle |
| `src-tauri/src/lib.rs` | `restart_process` komutu ekle |
| `src-tauri/permissions/commands.toml` | `restart_process` permission ekle |
| `src/App.vue` | "Yeniden Başlat" butonu + duplicate hata yönetimi ekle |
| `docs/wiki/Process-Yonetimi.md` | `restart_process` satırı ve duplicate davranış notu ekle |
