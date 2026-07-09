# Süreç Yönetimi (Process Management)

## Tauri Komutları

| Komut | Parametre | Dönüş | Açıklama |
|---|---|---|---|
| `get_processes` | `()` | `Vec<ProcessInfo>` | Tüm süreçleri listeler |
| `add_process` | `config: ProcessConfig` | `Result<(), String>` | Yeni süreç ekler |
| `update_process` | `id: String, config: ProcessConfig` | `Result<(), String>` | Süreç konfigürasyonunu günceller (`id` değiştirilemez) |
| `remove_process` | `id: String` | `Result<(), String>` | Süreci durdurur ve siler |
| `start_process` | `id: String` | `Result<(), String>` | Süreci manuel başlatır |
| `stop_process` | `id: String` | `Result<(), String>` | Süreci durdurur, DB'de `auto_start = 0` yapar |
| `restart_process` | `id: String` | `Result<(), String>` | Süreci durdurup yeniden başlatır (`Running`), veya direkt başlatır (`Stopped`/`Crashed`). Çalışma zamanı state'ini (PID, uptime) sıfırlar. `auto_start` ayarına dokunmaz. |
| `get_process_logs` | `id: String, max_lines: usize` | `Vec<String>` | Sürecin loglarını döndürür |

## Dinamik Event'ler

| Event | Payload | Açıklama |
|---|---|---|
| `log-line` | `{ id: String, line: String }` | Her stdout/stderr satırında tetiklenir |
| `status-changed` | `{ id: String, status: ProcessStatus }` | Süreç durumu değiştiğinde tetiklenir |

## ProcessConfig Alanları

| Alan | Tip | Açıklama |
|---|---|---|
| `id` | `String` | Benzersiz anahtar (primary key, değiştirilemez) |
| `name` | `String` | Görüntülenen ad |
| `command` | `String` | Çalıştırılabilir dosya yolu |
| `args` | `Vec<String>` | Komut argümanları |
| `cwd` | `Option<String>` | Çalışma dizini |
| `auto_restart` | `bool` | Crash durumunda otomatik yeniden başlatma |
| `max_restarts` | `usize` | Maksimum restart limiti |
| `auto_start` | `bool` | Uygulama açıldığında otomatik başlatma |

## ProcessStatus Değerleri

`Stopped | Running | Crashed | Restarting | Stopping`

## Davranış Notları

- Bir süreç manuel olarak durdurulduğunda (`stop_process`), DB'de `auto_start = 0` yapılır. Bu sayede uygulama yeniden başladığında süreç otomatik başlamaz.
- `update_process` çağrıldığında çalışan sürecin runtime state'i (PID, uptime, status) korunur. Süreç yeniden başlatılmaz.
- `id` alanı `update_process` ile değiştirilemez — parametreden gelen `id` esas alınır.
- `restart_process` çağrıldığında sürecin `auto_start` ayarına dokunulmaz. DB güncellenmez. Bu `stop_process`'ten farklıdır.
- Bir süreç başlatılmadan önce (`start_process` / `restart_process` Stopped/Crashed durumunda), sistemde aynı executable adına sahip başka bir süreç olup olmadığı kontrol edilir (`tasklist` ile). Eğer Guardian dışında bir süreç bulunursa hata döndürülür ve başlatma engellenir. Bu, aynı programın iki kere çalışmasını önler.
