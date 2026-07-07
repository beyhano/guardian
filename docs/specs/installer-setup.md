# Guardian — Windows Installer + Auto-Update Tasarımı

> **Tarih:** 2026-07-07
> **Durum:** Onaylandı

## 1. Amaç

Guardian uygulaması için Windows MSI installer oluşturmak ve GitHub Releases üzerinden
otomatik güncelleme (auto-update) altyapısını kurmak.

## 2. Yaklaşım

- **Installer:** Tauri v2 WiX MSI (built-in destek)
- **Güncelleme:** `tauri-plugin-updater` + GitHub Releases
- **CI/CD:** GitHub Actions — tag push ile tetiklenen build + release pipeline

## 3. MSI Installer (WiX)

### 3.1 Tauri Config

`tauri.conf.json` bundle bölümü:

```json
{
  "bundle": {
    "active": true,
    "targets": ["msi"],
    "createUpdaterArtifacts": true,
    "windows": {
      "wix": {
        "language": ["tr-TR", "en-US"]
      }
    }
  }
}
```

### 3.2 Özellikler

- **Kurulum dizini seçimi:** MSI standart Browse dialog'u ile gelir — ek yapılandırma gerekmez
- **Dil desteği:** Türkçe + İngilizce kurulum arayüzü
- **Kısayol:** Başlat menüsü + Masaüstü kısayolu (Tauri varsayılanı)
- **Kaldırma:** Windows Programs and Features üzerinden temiz kaldırma

### 3.3 İmzalama (Code Signing)

İlk sürüm için isteğe bağlı. Üretim dağıtımı için EV Code Signing sertifikası
önerilir, ayrı bir task olarak ertelenebilir.

## 4. Auto-Update Sistemi

### 4.1 Bileşenler

| Bileşen | Rol |
|---|---|
| `tauri-plugin-updater` | Rust tarafı — signature doğrulama, indirme, kurma |
| `@tauri-apps/plugin-updater` | Frontend — kullanıcıya bildirim |
| GitHub Releases | Update manifest + MSI dosyalarının barındığı yer |

### 4.2 Akış

```
Kullanıcı uygulamayı açar
       ↓
check() → GitHub Releases'daki latest.json sorgulanır
       ↓
Yeni sürüm var mı?
  ↓ EVET                ↓ HAYIR
  SweetAlert2 bildirimi   sessiz geç
       ↓
downloadAndInstall()
       ↓
relaunch() (tauri-plugin-process ile)
```

### 4.3 Anahtar Yönetimi

- `cargo tauri signer generate -w ~/.tauri/guardian-updater.key` ile keypair oluşturulur
- Public key → `tauri.conf.json` `plugins.updater.pubkey`
- Private key → GitHub Actions secret'ı (`TAURI_PRIVATE_KEY` + `TAURI_KEY_PASSWORD`)

### 4.4 Update Manifest

GitHub Release'e her build'de şu dosyalar eklenir:
- `guardian_0.1.0_x64.msi.zip` — sıkıştırılmış MSI
- `guardian_0.1.0_x64.msi.zip.sig` — imza
- `latest.json` — sürüm bilgisi (tauri-action otomatik üretir)

## 5. CI/CD Pipeline

### 5.1 GitHub Actions

Tek job, Windows runner:
1. Node.js + pnpm kurulumu
2. Rust + cache kurulumu
3. `pnpm install --frozen-lockfile`
4. `tauri-action` → build + release oluşturma

**Tetikleyici:** `v*` tag push'leri (ör: `v0.2.0`)

### 5.2 Gereken Secret'lar

| Secret | Kaynak |
|---|---|
| `GITHUB_TOKEN` | GitHub tarafından otomatik sağlanır |
| `TAURI_PRIVATE_KEY` | `cargo tauri signer generate` çıktısı |
| `TAURI_KEY_PASSWORD` | Key oluşturulurken belirlenen şifre |

## 6. Değişen Dosyalar

```
YENI: .github/workflows/release.yml
YENI: src/components/UpdateManager.vue        # Güncelleme UI bileşeni
DEGISECEK: src-tauri/Cargo.toml               # tauri-plugin-updater
DEGISECEK: src-tauri/tauri.conf.json          # WiX + updater config
DEGISECEK: src-tauri/capabilities/default.json # updater:default permission
DEGISECEK: package.json                       # @tauri-apps/plugin-updater
```

## 7. Test Planı

1. **MSI test:** `pnpm tauri build` → çıkan `.msi` dosyasını çalıştır
2. **Kurulum dizini:** Browse ile farklı dizin seç, kurulumun çalıştığını doğrula
3. **Kaldırma:** Programs and Features'den kaldır, kalıntı kalmasın
4. **Güncelleme:** GitHub'da draft release oluştur, uygulamadan kontrol et
5. **Hata senaryoları:** İnternet yokken güncelleme kontrolü, imza hatası, disk dolu

## 8. Yapılmayanlar (Out of Scope)

- EV Code Signing sertifikası alma ve imzalama
- macOS / Linux installer'ları
- WiX template ile özel UI özelleştirmeleri
- Incremental/Delta güncellemeler
