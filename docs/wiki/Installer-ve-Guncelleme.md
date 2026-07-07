# Windows Installer ve Auto-Update Sistemi

> **Tarih:** 2026-07-07
> **İlgili:** [[Index]], [[RELEASE.md]]

---

## 1. Mimari Genel Bakış

Guardian uygulaması Tauri v2 ile inşa edilmiştir. Windows'ta dağıtım için **WiX MSI** installer kullanılır. Güncellemeler **GitHub Releases** üzerinden imzalı olarak dağıtılır.

```
+------------------+       +--------------------+       +------------------+
|  GitHub Actions  | ----> |  GitHub Release    | <---- |  Kullanıcı       |
|  (build + sign)  |       |  (MSI + .sig +    |       |  (app check())   |
|                  |       |   latest.json)     |       |                  |
+------------------+       +--------------------+       +------------------+
        |                                                        |
        | pnpm tauri build                                       | check()
        v                                                        v
+------------------+                                     +------------------+
|  Rust Derlemesi  |                                     |  UpdateManager   |
|  WiX MSI Paketi  |                                     |  (Vue Component) |
|  Updater İmzalama|                                     |  + SweetAlert2   |
+------------------+                                     +------------------+
```

---

## 2. MSI Installer (WiX)

### 2.1 Yapılandırma

`src-tauri/tauri.conf.json`:

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

### 2.2 Özellikler

| Özellik | Açıklama |
|---------|----------|
| **Format** | MSI (Windows Installer) |
| **Kurulum dizini** | Kullanıcı Browse ile seçebilir (WiX varsayılanı) |
| **Dil** | Türkçe + İngilizce (sistem diline göre otomatik seçilir) |
| **Kısayol** | Başlat menüsü (Tauri varsayılanı) |
| **Kaldırma** | Programs and Features üzerinden temiz kaldırma |
| **Boyut** | ~5.5 MB |

### 2.3 Build Komutu

```bash
pnpm tauri build --bundles msi
```

Çıktılar:
```
src-tauri/target/release/bundle/msi/
├── guardian_0.1.0_x64_tr-TR.msi       # Türkçe MSI
├── guardian_0.1.0_x64_tr-TR.msi.sig   # İmza
├── guardian_0.1.0_x64_en-US.msi       # İngilizce MSI
└── guardian_0.1.0_x64_en-US.msi.sig   # İmza
```

---

## 3. Auto-Update Sistemi

### 3.1 Bileşenler

| Katman | Paket | Görev |
|--------|-------|-------|
| Rust | `tauri-plugin-updater = "2"` | İmza doğrulama, indirme, kurma |
| Rust | `tauri-plugin-process = "2"` | `relaunch()` için |
| Frontend | `@tauri-apps/plugin-updater` | `check()` API'si |
| Frontend | `@tauri-apps/plugin-process` | `relaunch()` API'si |

### 3.2 Güncelleme Akışı

```
Uygulama açılır
    ↓
5 saniye bekler (IPC'nin hazır olması için)
    ↓
check() → GitHub Releases'daki latest.json sorgulanır
    ↓
Yeni sürüm VAR mı?
    ↓ EVET                        ↓ HAYIR / HATA
SweetAlert2 bildirimi           Sessiz geç (arka plan)
    ↓
Kullanıcı "Güncelle" derse
    ↓
downloadAndInstall()
    ↓
relaunch() — uygulama kapanır, yenisi açılır
```

### 3.3 Manuel Kontrol

Header'daki 🔄 butonu ile kullanıcı istediği zaman güncelleme kontrolü yapabilir. Manuel kontrolde hata olursa SweetAlert2 ile kullanıcıya bildirilir.

### 3.4 Periyodik Kontrol

Uygulama açıkken **6 saatte bir** arka planda güncelleme kontrolü yapılır. Sadece güncelleme bulunursa kullanıcıya bildirilir.

```typescript
// src/components/UpdateManager.vue
onMounted(() => {
  autoCheckTimer = setTimeout(() => checkForUpdates(true), 5000)
  periodicTimer = setInterval(() => checkForUpdates(true), 6 * 60 * 60 * 1000)
})
```

---

## 4. İmzalama (Signing)

### 4.1 Keypair Oluşturma

```bash
pnpm tauri signer generate -w "$env:USERPROFILE\.tauri\guardian-updater.key" --password "şifreniz"
```

Çıktılar:
```
C:\Users\beyhan\.tauri\
├── guardian-updater.key          # ÖZEL anahtar (GİZLİ)
└── guardian-updater.key.pub      # Açık anahtar (tauri.conf.json'a yazılır)
```

### 4.2 Public Key Konumu

`src-tauri/tauri.conf.json` → `plugins.updater.pubkey` alanına yazılır.

### 4.3 Private Key Güvenliği

- **ASLA** repo'ya commit edilmez
- **GitHub Secret** olarak saklanır: `Settings > Secrets and variables > Actions`
- GitHub'da 2 secret gerekli:
  - `TAURI_PRIVATE_KEY` = `guardian-updater.key` dosyasının tüm içeriği
  - `TAURI_KEY_PASSWORD` = key oluşturulurken girilen şifre

### 4.4 Local Build İçin

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\guardian-updater.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "şifreniz"
pnpm tauri build --bundles msi
```

---

## 5. CI/CD Pipeline

### 5.1 Workflow

`.github/workflows/release.yml`:

```yaml
on:
  push:
    tags:
      - 'v*'        # v0.1.0, v0.2.0, ...
```

### 5.2 Yaptıkları

1. `actions/checkout@v4` — repo'yu çeker
2. Node.js + pnpm kurar
3. Rust kurar + cache'ler
4. `pnpm install --frozen-lockfile` — bağımlılıkları yükler
5. `tauri-apps/tauri-action@v1` — build alır, imzalar, release oluşturur

### 5.3 Release Türü

- **Draft release** oluşturur (herkese açık değil)
- **Manuel publish** gerekir — GitHub Releases sayfasından
- Publish edilince `releases/latest` aktif olur, kullanıcılar güncellemeyi görür

### 5.4 Upload Edilen Dosyalar

- `guardian_0.1.0_x64_tr-TR.msi`
- `guardian_0.1.0_x64_tr-TR.msi.sig`
- `guardian_0.1.0_x64_en-US.msi`
- `guardian_0.1.0_x64_en-US.msi.sig`
- `latest.json` (updater manifest'i)

---

## 6. Release Süreci

### 6.1 PowerShell Script (Önerilen)

```powershell
.\release.ps1 -Version 0.2.0
```

Script yaptıkları:
1. `pnpm build` + `cargo check` ile doğrulama
2. `tauri.conf.json`, `package.json`, `Cargo.toml` versiyon güncelleme
3. Git commit + tag + push
4. GitHub Actions tetiklenir

### 6.2 Manuel (Scriptsiz)

```bash
# 1. Versiyonları güncelle (tauri.conf.json, package.json, Cargo.toml)
# 2. CHANGELOG.md'ye not ekle
# 3. Commit + tag + push
git add -A
git commit -m "chore: v0.2.0 release"
git tag v0.2.0
git push origin main
git push origin v0.2.0
```

### 6.3 Release Yayınlama

1. Build bitince `https://github.com/beyhano/guardian/actions` kontrol et
2. Başarılı olunca `https://github.com/beyhano/guardian/releases` sayfasına git
3. Draft release bul, **Publish release** yap

---

## 7. Önemli Uyarılar

### Windows'ta `~` Kullanımı

Windows PowerShell'de `~` **HOME dizinine gitmez**. `%USERPROFILE%` veya `$env:USERPROFILE` kullan.

```powershell
# YANLIŞ: ~\.tauri\guardian-updater.key  (repo içinde literal ~ dizini oluşur)
# DOĞRU: $env:USERPROFILE\.tauri\guardian-updater.key
```

### Env Var İsimleri

Tauri v2'de env var isimleri **farklıdır**:

| Amaç | Tauri v1 | Tauri v2 (doğru) |
|------|----------|-----------------|
| Private key | `TAURI_PRIVATE_KEY` | `TAURI_SIGNING_PRIVATE_KEY` |
| Password | `TAURI_KEY_PASSWORD` | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |

### tauri-action Versiyonu

`tauri-apps/tauri-action@v1` Tauri v2 ile çalışır. `@v2` gerekmez.

### API Farkı

`tauri-plugin-updater` v2.10.1 API'sinde `init()` yoktur:

```rust
// YANLIŞ: .plugin(tauri_plugin_updater::init())
// DOĞRU:  .plugin(tauri_plugin_updater::Builder::new().build())
```

---

## 8. Klasör Yapısı

```
guardian/
├── .github/workflows/
│   └── release.yml              # CI/CD pipeline
├── src/
│   └── components/
│       └── UpdateManager.vue     # Güncelleme UI bileşeni
├── src-tauri/
│   ├── Cargo.toml                # tauri-plugin-updater, tauri-plugin-process
│   ├── tauri.conf.json           # MSI + WiX + updater config
│   ├── capabilities/
│   │   └── default.json          # updater:default permission
│   └── src/
│       └── lib.rs                # Plugin kayıtları
├── release.ps1                   # Release script'i
├── RELEASE.md                    # Release talimatları
├── CHANGELOG.md                  # Sürüm notları
└── docs/wiki/
    └── Installer-ve-Guncelleme.md  # Bu doküman
```

---

## 9. Sık Sorulan Sorular

### Build local'de çalışıyor ama CI'da hata veriyor?

CI'da `TAURI_SIGNING_PRIVATE_KEY` ortam değişkeni eksik olabilir. GitHub Actions'ta `TAURI_PRIVATE_KEY` secret'ını kontrol et.

### Release oluştu ama kullanıcı görmüyor?

Draft release henüz publish edilmemiştir. GitHub Releases sayfasından **Publish release** yap.

### Güncelleme kontrolü "Başarısız" döndü?

- GitHub'da release publish edilmiş olmalı
- `latest.json` URL'si doğru olmalı (`beyhano/guardian`)
- Public key, private key ile eşleşmeli

### Windows'ta "Bu uygulama güvenli değil" uyarısı?

Bu normal — kod imzalama (EV Code Signing) sertifikası alınmadığı sürece Windows SmartScreen uyarı gösterir. İleride sertifika eklenebilir.
