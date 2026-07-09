# Guardian - Windows Süreç Yöneticisi Geliştirme Günlüğü

Bu dosyada, Guardian (Windows Supervisor) projesi kapsamında geliştirilen tüm özellikler, teknik kararlar, mimari değişiklikler ve tamamlanan görevler kronolojik ve modüler olarak özetlenmiştir.

---

## 🚀 Gerçekleştirilen Özellikler ve Modüller

### 1. Cargo ve npm Bağımlılıkları (En Son Kararlı Versiyonlar)
* **Rust (Cargo)**:
  * `tokio` (Asenkron runtime ve süreç kontrolü)
  * `chrono` (Zaman damgalı loglama ve çalışma süreleri)
  * `tauri-plugin-dialog` (Yerel dosya seçme penceresi entegrasyonu)
  * `rusqlite` (SQLite veritabanı sürücüsü, `"bundled"` özelliğiyle eklenerek sistem bağımlılığı olmadan taşınabilir derleme sağlandı)
  * `serde` & `serde_json` (Serileştirme ve JSON dönüşümleri)
* **Frontend (npm)**:
  * `@tauri-apps/plugin-dialog` (Dosya seçici frontend köprüsü)
  * `sweetalert2` (Koyu tema uyumlu, animasyonlu modern onay ve bildirim pencereleri)

### 2. Rust Backend Süreç Yöneticisi (`process_manager.rs`)
* **Arka Plan Süreç Yönetimi**: `tokio::process::Command` kullanılarak süreçler Windows konsol pencereleri açılmadan (`CREATE_NO_WINDOW` bayrağıyla) tamamen arka planda başlatılır.
* **Otomatik Yeniden Başlatma (Auto-Recovery)**: Çöken veya sıfırdan farklı exit code ile kapanan süreçler, belirlenen limit doğrultusunda 2 saniye gecikmeli olarak otomatik olarak yeniden başlatılır.
* **Ağaç Bazlı Süreç Sonlandırma**: Süreçler durdurulurken yetim (orphan) alt süreçlerin kalmaması için Windows `taskkill /F /T` komutu asenkron olarak çağrılarak tüm süreç ağacı güvenli bir şekilde kapatılır.

### 3. SQLite Veritabanı Katmanı
* Tüm süreç konfigürasyonları diskteki JSON dosyası yerine yerel SQLite veritabanında (`guardian.db`) saklanır.
* Uygulama ilk açıldığında `init_db` metodu ile veritabanı şeması otomatik hazırlanır ve kaydedilen tüm süreçler `load_configs` ile okunarak belleğe yüklenir.
* `auto_start` özelliği işaretli olan süreçler, uygulama açıldığı anda otomatik olarak başlatılır.

### 4. Performans Odaklı Asenkron Loglama (Tail Reader)
* Her sürecin standart çıktı (`stdout`) ve hata çıktıları (`stderr`) toplanıp asenkron olarak `<app_data_dir>/logs/<process_id>.log` dosyasına yazılır.
* Log dosyasının son `N` satırını diskten okuyan geriye doğru kaydırmalı (seek) ve performans odaklı tail okuyucu yazıldı.
* Log takibi için webview tarafına Tauri event'leri (`log-line`) asenkron olarak gönderilir.

### 5. Vue 3 + TypeScript Frontend ve Yeni Dikey Düzen
* **Dikey Dashboard Tasarımı**:
  * **Üst Bölüm (Süreç Listesi)**: Süreçlerin listelendiği, durum bilgilerinin ve butonların yer aldığı alan üst bölüme taşındı. Süreç kartları geniş ekranlarda yan yana dizilecek şekilde esnek bir ızgara (`grid`) yapısına kavuşturuldu (`320px - 400px` sınırıyla).
  * **Alt Bölüm (Log Terminali)**: Log terminali tam genişlikte alt bölüme alınarak geniş ekran log izleme alanı sağlandı.
  * **İstatistik Paneli**: Toplam, Çalışan, Çöken ve Durdurulan süreç adetlerini gösteren üst bar eklendi. İstatistik kartlarının minimum genişliği `120px` değerine indirilerek tek satırda hizalanması sağlandı.
* **Gözat Butonu**: Süreç ekleme formundaki komut alanının yanına native dosya seçme penceresini tetikleyen `Gözat...` butonu eklendi. Kullanıcılar `.exe`, `.bat`, `.cmd` gibi çalıştırılabilir dosyaları kolayca seçebilir.
* **Pencere Boyut Sınırları**: Kullanıcının uygulamayı kullanılmaz derecede küçültmesini önlemek için `tauri.conf.json` üzerinde minimum pencere boyutları (`minWidth: 750`, `minHeight: 500`) kilitlendi.

### 6. SweetAlert2 Onay ve Bildirim Sistemi
* Tarayıcının standart ve çirkin `alert` ile `confirm` pencereleri tamamen kaldırıldı.
* **Sil**, **Başlat** ve **Durdur** eylemlerine tıklanıldığında kazara eylemleri önlemek amacıyla koyu tema uyumlu SweetAlert2 onay kutuları eklendi.
* Butonların kenar yuvarlaklıkları, renk tonları ve gölgeleri uygulamanın genel cam (glassmorphism) tasarımına entegre edildi.

---

## 📂 Dosya ve Dizin Konumları (Windows)

Tüm kullanıcı verileri ve veritabanı Windows'un standart AppData dizininde barındırılır:

* **Süreç Veritabanı**: `C:\Users\<Kullanıcı>\AppData\Roaming\com.beyhan.guardian\guardian.db`
* **Sürece Özel Loglar**: `C:\Users\<Kullanıcı>\AppData\Roaming\com.beyhan.guardian\logs\<process_id>.log`

---

## 📦 Üretilen Paketler (Release Builds)

`pnpm tauri build` derleme komutu başarıyla tamamlanmış ve aşağıdaki kurulum paketleri üretilmiştir:
* **Windows MSI Kurulum Paketi**: `src-tauri\target\release\bundle\msi\guardian_0.1.0_x64_en-US.msi`
* **Windows NSIS Kurulum Sihirbazı (Setup.exe)**: `src-tauri\target\release\bundle\nsis\guardian_0.1.0_x64-setup.exe`

---

## 🔮 Gelecek Adımlar

1. **Süreç Metrikleri**: Süreçlerin bellek (RAM) ve işlemci (CPU) kullanımlarının anlık olarak takip edilip süreç kartlarında gösterilmesi.
2. **Kullanıcı Bildirimleri (Toast)**: Hatalar veya çökmeler yaşandığında işletim sistemine native bildirim gönderilmesi.
3. **Log İndirme/Temizleme**: Log dosyasını doğrudan arayüzden `.txt` olarak indirme ve veritabanından log arşivleme seçenekleri.

---

## 🔄 Restart Process + Duplicate Execution Prevention (2026-07-09)

### 7. `restart_process` Backend Komutu (`process_manager.rs`)
* `ProcessManager::restart_process(id)` metodu eklendi — süreci **yeniden başlatma** işlemi:
  * **Running →** `stop_tx` sinyali + `taskkill /F /T /PID` ile durdur → 200ms bekle → `spawn_process_internal` ile yeniden başlat
  * **Stopped / Crashed →** direkt `spawn_process_internal` (öncesinde duplicate kontrolü)
  * **Restarting / Stopping →** hata döndür (`"Process is already restarting/stopping"`)
* **Önemli:** `stop_process`'ten farklı olarak DB'de `auto_start` ayarına **dokunulmaz**. Restart geçici bir işlemdir, yapılandırmayı bozmaz.

### 8. Duplicate Çalıştırma Koruması (`process_manager.rs`)
* `check_external_duplicate(command)` metodu eklendi:
  * `command` yolundan executable adını çıkarır
  * Windows'ta `tasklist /NH /FO CSV /FI "IMAGENAME eq <exe>"` ile sistem sorgulanır
  * Guardian'ın bildiği PID'ler dışında bir instance bulunursa `Err` döndürülür
  * Başarısız olursa sessizce geçer (best-effort guard)
* `start_process` çağrılmadan önce duplicate kontrolü yapar — aynı programın iki kere başlatılmasını engeller

### 9. Frontend "Yeniden Başlat" Butonu (`App.vue`)
* Running ve Crashed durumlarındaki süreçlerde **🔄 Yeniden Başlat** butonu gösterilir
* Buton sırası: ▶ Başlat / ■ Durdur → 🔄 Yeniden Başlat → 📋 Loglar → ✏️ Düzenle → 🗑️ Sil
* Doğrudan `restart_process` invoke eder, onay sormaz (arka planda stop+start yapar)
* Duplicate hatası durumunda SweetAlert2 uyarısı: `"Program Zaten Çalışıyor!"`

### 10. Wiki Güncellemesi (`docs/wiki/Process-Yonetimi.md`)
* Tauri Komutları tablosuna `restart_process` satırı eklendi
* Davranış notları: restart `auto_start`'a dokunmaz, duplicate kontrolü çalışır

**İlgili Commit'ler:** `f780579`, `dface58`, `cc273b9`, `faa5854`, `3de4387`

---

## 🔄 Duplicate Hatasında Durdur ve Başlat (2026-07-09)

### 11. `force_start_process` Backend Komutu (`process_manager.rs` + `lib.rs`)
* `ProcessManager::force_start_process(id)` metodu eklendi — `taskkill /F /T /IM` ile tüm instance'ları öldürür, 500ms bekler, duplicate kontrolü yapmadan doğrudan spawn eder.
* `kill_external_pid(PID)` de eklendi ama gerek kalmadı — `/PID` yerine `/IM` (image name) daha güvenilir.
* `commands.toml` izin listesine `force_start_process` eklendi.

### 12. Frontend Duplicate Dialog İyileştirmesi (`App.vue`)
* **Başlat** ve **Yeniden Başlat** dialogunda: "🔫 Var Olanı Durdur ve Başlat" butonu, PID parse etmeden doğrudan `force_start_process` çağırır.
* PID'den bağımsız çalışır — yeni instance farklı PID'de başlasa bile `/IM` ile hepsi öldürülür.

### 13. Arka Plan Duplicate Süreç İzleyicisi (`process_manager.rs` + `App.vue`)
* `start_duplicate_monitor()` — her 10 saniyede bir, Running durumundaki süreçlerin sistemde Guardian dışında da çalışıp çalışmadığını kontrol eder.
* Tespit edilirse frontend'e `duplicate-detected` event'i gönderilir.
* Frontend SweetAlert2 ile uyarı gösterir: "👥 Dış Süreç Tespit Edildi! PID X sistemde Guardian dışında çalışıyor."
* Aynı süreç için aynı oturumda tekrar bildirimi önlemek için `duplicateNotified` Set cooldown.

**İlgili Commit'ler:** `f780579`, `dface58`, `cc273b9`, `faa5854`, `3de4387`
