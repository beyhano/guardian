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
| `force_start_process` | `id: String` | `Result<(), String>` | Duplicate kontrolü YAPMADAN süreci başlatır. Önce `taskkill /F /T /IM` ile tüm instance'ları öldürür, sonra spawn eder. "Program Zaten Çalışıyor!" dialogundaki "Durdur ve Başlat" butonu için. |
| `kill_external_pid` | `pid: u32` | `Result<(), String>` | Sistemdeki dış bir süreci PID ile zorla sonlandırır (taskkill → PowerShell → wmic fallback). |
| `get_process_logs` | `id: String, max_lines: usize` | `Vec<String>` | Sürecin loglarını döndürür |

## Dinamik Event'ler

| Event | Payload | Açıklama |
|---|---|---|
| `log-line` | `{ id: String, line: String }` | Her stdout/stderr satırında tetiklenir |
| `status-changed` | `{ id: String, status: ProcessStatus }` | Süreç durumu değiştiğinde tetiklenir |
| `duplicate-detected` | `{ id: String, exe_name: String, external_pids: Vec<u32> }` | Guardian dışında çalışan aynı executable tespit edildiğinde tetiklenir (10sn periyodik kontrol) |

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
- `force_start_process` duplicate kontrolünü atlar — önce `taskkill /F /T /IM` ile tüm instance'ları öldürür, sonra başlatır. "Program Zaten Çalışıyor!" hatasında "Durdur ve Başlat" seçeneği olarak sunulur.
- Arka planda her 10 saniyede bir `tasklist` sorgulanarak, Running durumundaki süreçlerin Guardian dışında da çalışıp çalışmadığı kontrol edilir. Tespit edilirse frontend'e `duplicate-detected` event'i gönderilir.
