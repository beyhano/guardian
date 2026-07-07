# Changelog

## v0.1.1 — 2026-07-08

### Eklemeler
- Süreç düzenleme (edit) — mevcut süreçlerin konfigürasyonu düzenlenebilir
- `update_process` Tauri komutu — `id` sabit, diğer tüm alanlar değiştirilebilir
- Otomatik CWD algılama — `.env` ve göreceli yollar için command dizini otomatik kullanılır
- `docs/wiki/Process-Yonetimi.md` — süreç yönetimi API dokümantasyonu

### Düzeltmeler
- `stop_process` artık DB'de `auto_start = 0` yapar — manuel durdurulan süreç restart'ta başlamaz

## v0.1.0 — 2026-07-07

### Eklemeler
- Windows MSI installer (WiX) — kurulum dizini seçme desteği
- Tauri updater entegrasyonu — GitHub Releases üzerinden auto-update
- Güncelleme bildirim UI'ı — SweetAlert2 ile kullanıcıya bildirim
- GitHub Actions CI/CD — tag push ile otomatik build + release

### Teknik
- Tauri v2'den v2.11.x'e güncelleme
- tauri-plugin-updater ile imzalı güncelleme desteği
- WiX MSI ile çift dil desteği (Türkçe + İngilizce)
