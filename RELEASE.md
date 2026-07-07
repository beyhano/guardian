# Release Süreci

## Hızlı Başlangıç

Yeni bir sürüm çıkarmak için:

```powershell
.\release.ps1 -Version 0.2.0
```

Script şunları yapar:
1. `pnpm build` + `cargo check` ile her şeyin derlendiğini doğrular
2. Versiyon numarasını `tauri.conf.json`, `package.json`, `Cargo.toml`'da günceller
3. Commit atar
4. `v0.2.0` tag'ini oluşturup push'lar
5. GitHub Actions otomatik build alıp **draft release** oluşturur

---

## Adım Adım

### 1. CHANGELOG'u Güncelle

`CHANGELOG.md` dosyasına yeni sürümün notlarını ekle:

```markdown
## v0.2.0 — 2026-07-07

### Eklemeler
- [yeni özellik]

### Düzeltmeler
- [fix]
```

### 2. Release Script'i Çalıştır

```powershell
cd C:\Users\beyhan\Desktop\Projeler\Rust\guardian
.\release.ps1 -Version 0.2.0
```

Eğer build'i local'de almak istemezsen (GitHub Actions yapsın):

```powershell
.\release.ps1 -Version 0.2.0 -SkipBuild
```

### 3. GitHub Actions'ı Bekle

Build tetiklendiğinde şu adresten takip edebilirsin:

```
https://github.com/beyhano/guardian/actions
```

Yaklaşık 5-10 dakika sürer.

### 4. Draft Release'i Yayınla

Build bitince şu adreste bir **draft release** oluşur:

```
https://github.com/beyhano/guardian/releases
```

1. Draft release'e tıkla
2. "Edit" ile release notes'u kontrol et (veya düzenle)
3. **Publish release** butonuna bas

Artık kullanıcılar uygulamayı açtığında yeni sürümü görüp güncelleyebilir.

---

## İlk Sürüm (v0.1.0) — Manuel

İlk sürüm için `release.ps1` henüz yokken manuel yapıldı:

```bash
git add -A
git commit -m "chore: v0.1.0 release"
git tag v0.1.0
git push origin main
git push origin v0.1.0
```

Bundan sonra hep `.\release.ps1` kullan.

---

## Gereken GitHub Secret'lar

| Secret | Değer |
|--------|-------|
| `TAURI_PRIVATE_KEY` | `C:\Users\beyhan\.tauri\guardian-updater.key` dosyasının tüm içeriği |
| `TAURI_KEY_PASSWORD` | Key oluşturulurken girilen şifre |

Bunlar `Settings > Secrets and variables > Actions` sayfasında ayarlanır.

---

## Mimari

```
Git tag push (v*)
       ↓
GitHub Actions (windows-latest)
       ↓
pnpm tauri build --bundles msi
       ↓
MSI + .sig + latest.json
       ↓
GitHub Release (draft)
       ↓
[Manuel] Publish release
       ↓
Kullanıcı uygulamayı açınca check()
       ↓
Yeni sürüm varsa → SweetAlert2 bildirimi → downloadAndInstall → relaunch
```
