# Guardian — Windows Installer + Auto-Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guardian uygulaması için WiX MSI installer oluşturmak ve GitHub Releases üzerinden auto-update altyapısını kurmak.

**Architecture:** Tauri v2'nin built-in WiX MSI desteği + `tauri-plugin-updater` + GitHub Actions CI/CD. Build'ler tag push ile tetiklenir, tauri-action MSI + updater artifact'lerini GitHub Release'e yükler.

**Tech Stack:** Tauri v2, Rust, Vue 3, GitHub Actions, WiX, MSI

## Global Constraints

- Proje pnpm kullanır (npm değil)
- Windows-only MSI hedefi (`bundle.targets: ["msi"]`)
- Tüm kod/yorum/commit mesajları Türkçe
- `unsafe` Rust kullanımı yasak
- Her değişiklikten sonra `cargo check` ve `cargo test` zorunlu

---

### Task 1: Updater Plugin — Rust Tarafı + Tauri Config

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: mevcut `tauri.conf.json` bundle config, mevcut `Cargo.toml`
- Produces: updater plugin entegre edilmiş Tauri projesi

- [ ] **Step 1: `tauri-plugin-updater` bağımlılığını ekle**

`src-tauri/Cargo.toml` dosyasında `[dependencies]` bölümüne şu satırı ekle:

```toml
tauri-plugin-updater = "2"
```

- [ ] **Step 2: `@tauri-apps/plugin-updater` frontend paketini yükle**

```bash
pnpm add @tauri-apps/plugin-updater
```

- [ ] **Step 3: `tauri.conf.json` — MSI + updater config**

`src-tauri/tauri.conf.json` dosyasını aşağıdaki gibi güncelle:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "guardian",
  "version": "0.1.0",
  "identifier": "com.beyhan.guardian",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "guardian",
        "width": 800,
        "height": 600,
        "minWidth": 750,
        "minHeight": 500
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi"],
    "createUpdaterArtifacts": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "wix": {
        "language": ["tr-TR", "en-US"]
      }
    }
  },
  "plugins": {
    "updater": {
      "pubkey": "",
      "endpoints": [
        "https://github.com/beyhan/guardian/releases/latest/download/latest.json"
      ]
    }
  }
}
```

> `pubkey` alanı Task 4'te oluşturulacak anahtar ile doldurulacak.

- [ ] **Step 4: `capabilities/default.json` — updater permission**

`src-tauri/capabilities/default.json` dosyasını oku ve `permissions` dizisine `"updater:default"` ekle:

```json
{
  "identifier": "default",
  "description": "Varsayılan yetkiler",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:default",
    "updater:default"
  ]
}
```

- [ ] **Step 5: Plugin kaydı — `lib.rs`**

`src-tauri/src/lib.rs` dosyasını oku. `tauri::Builder::default()` içinde `.plugin(tauri_plugin_updater::init())` çağrısını ekle.

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::init())  // yeni satır
        .invoke_handler(tauri::generate_handler![
            // mevcut handler'lar
        ])
        .setup(|app| {
            // mevcut setup
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> Mevcut `.plugin()` ve `.invoke_handler()` çağrılarını koru, sadece updater satırını ekle.

- [ ] **Step 6: Derlemeyi test et**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: `Finished` — hata yok.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: tauri-plugin-updater entegrasyonu ve MSI yapılandırması"
```

---

### Task 2: Frontend — UpdateManager Bileşeni

**Files:**
- Create: `src/components/UpdateManager.vue`
- Modify: `src/App.vue` (UpdateManager'i mount et)

**Interfaces:**
- Consumes: `@tauri-apps/plugin-updater` (Task 1'de yüklendi)
- Consumes: `@tauri-apps/plugin-process` (gerekli — `pnpm add @tauri-apps/plugin-process`)
- Produces: Kullanıcıya güncelleme bildirimi gösteren Vue bileşeni

- [ ] **Step 1: `@tauri-apps/plugin-process` paketini yükle**

```bash
pnpm add @tauri-apps/plugin-process
```

- [ ] **Step 2: `UpdateManager.vue` bileşenini oluştur**

`src/components/UpdateManager.vue`:

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import Swal from 'sweetalert2'

const checking = ref(false)

async function checkForUpdates() {
  if (checking.value) return
  checking.value = true

  try {
    const update = await check()

    if (update?.available) {
      const result = await Swal.fire({
        title: 'Güncelleme Mevcut',
        html: `
          <p><strong>v${update.version}</strong> kullanılabilir.</p>
          <p style="font-size:0.85em;color:#888;margin-top:8px;">${update.body || ''}</p>
          <p style="margin-top:12px;">Şimdi güncellemek istiyor musunuz?</p>
        `,
        icon: 'info',
        showCancelButton: true,
        confirmButtonText: 'Güncelle',
        cancelButtonText: 'Daha Sonra',
        confirmButtonColor: '#6c5ce7',
        background: '#1a1a2e',
        color: '#eee',
      })

      if (result.isConfirmed) {
        await Swal.fire({
          title: 'Güncelleme İndiriliyor...',
          html: '<div class="spinner"></div>',
          allowOutsideClick: false,
          showConfirmButton: false,
          background: '#1a1a2e',
          color: '#eee',
        })

        await update.downloadAndInstall()
        await relaunch()
      }
    } else {
      await Swal.fire({
        title: 'Güncelleme Yok',
        text: 'Zaten en son sürümü kullanıyorsunuz.',
        icon: 'success',
        timer: 3000,
        showConfirmButton: false,
        background: '#1a1a2e',
        color: '#eee',
      })
    }
  } catch (err) {
    console.error('Güncelleme kontrolü başarısız:', err)
  } finally {
    checking.value = false
  }
}

onMounted(() => {
  // Uygulama açıldığında arka planda kontrol et (3 saniye gecikmeli)
  setTimeout(() => checkForUpdates(), 3000)
})
</script>

<template>
  <button
    class="update-btn"
    :disabled="checking"
    @click="checkForUpdates"
    :title="checking ? 'Kontrol ediliyor...' : 'Güncellemeleri Kontrol Et'"
  >
    <span v-if="checking" class="spinner-small"></span>
    <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M23 4v6h-6M1 20v-6h6"/>
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
    </svg>
  </button>
</template>

<style scoped>
.update-btn {
  background: rgba(108, 92, 231, 0.15);
  border: 1px solid rgba(108, 92, 231, 0.3);
  border-radius: 8px;
  color: #a29bfe;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  transition: all 0.2s;
}
.update-btn:hover:not(:disabled) {
  background: rgba(108, 92, 231, 0.3);
  border-color: #6c5ce7;
}
.update-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.spinner-small {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(108, 92, 231, 0.3);
  border-top-color: #6c5ce7;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
```

- [ ] **Step 3: `UpdateManager`'i `App.vue`'ye entegre et**

`src/App.vue` dosyasını oku. Uygun bir konuma (örneğin header toolbar'ın sağ üst köşesine) UpdateManager bileşenini ekle:

```vue
<script setup lang="ts">
import UpdateManager from './components/UpdateManager.vue'
// ... mevcut importlar
</script>

<template>
  <!-- Mevcut template'in uygun bir yerine, örneğin header'a: -->
  <div class="header-actions">
    <UpdateManager />
    <!-- mevcut butonlar -->
  </div>
</template>
```

> Var olan template yapısını bozmadan UpdateManager'i header toolbar'ın sağ tarafına yerleştir.

- [ ] **Step 4: TypeScript kontrolü**

```bash
pnpm build
```

Expected: `Completed` — tip hatası yok.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: UpdateManager bileşeni ve güncelleme UI'ı"
```

---

### Task 3: GitHub Actions CI/CD Pipeline

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: Task 1'de yapılandırılmış `tauri.conf.json` (createUpdaterArtifacts: true)
- Produces: Tag push'te otomatik MSI + updater artifact üreten pipeline

- [ ] **Step 1: `.github/workflows/release.yml` oluştur**

```yaml
name: 'guardian-release'

on:
  push:
    tags:
      - 'v*'

jobs:
  create-release:
    runs-on: windows-latest
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Node.js kurulumu
        uses: actions/setup-node@v4
        with:
          node-version: lts/*

      - name: pnpm kurulumu
        uses: pnpm/action-setup@v4
        with:
          version: latest

      - name: Rust kurulumu
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Frontend bağımlılıklarını yükle
        run: pnpm install --frozen-lockfile

      - name: Tauri build + release oluştur
        uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
          TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
        with:
          tagName: v__VERSION__
          releaseName: 'Guardian v__VERSION__'
          releaseBody: 'Yenilikler için CHANGELOG.md dosyasına bakın.'
          releaseDraft: true
```

- [ ] **Step 2: Commit**

```bash
git add -A
git commit -m "ci: GitHub Actions release pipeline"
```

---

### Task 4: Signing Key Oluşturma + Build Testi

**Files:**
- Modify: `src-tauri/tauri.conf.json` (pubkey alanını doldur)
- Output: `~/.tauri/guardian-updater.key` ve `.key.pub`

**Interfaces:**
- Consumes: Tauri CLI
- Produces: İmzalama anahtarları, güncellenmiş `tauri.conf.json`

- [ ] **Step 1: Updater signing keypair oluştur**

```bash
cargo tauri signer generate -w ~/.tauri/guardian-updater.key
```

Komut çalıştırıldığında:
1. Bir şifre (password) soracak — güçlü bir şifre gir ve kaydet
2. `~/.tauri/guardian-updater.key` (private key) oluşacak
3. `~/.tauri/guardian-updater.key.pub` (public key) oluşacak

- [ ] **Step 2: Public key'i `tauri.conf.json`'a ekle**

`.key.pub` dosyasının içeriğini oku ve `tauri.conf.json`'daki `plugins.updater.pubkey` alanına yapıştır.

- [ ] **Step 3: Build testi yap**

```bash
pnpm tauri build
```

Expected:
- `src-tauri/target/release/bundle/msi/guardian_0.1.0_x64.msi` — MSI dosyası
- `src-tauri/target/release/bundle/msi/guardian_0.1.0_x64.msi.zip` — sıkıştırılmış MSI
- `src-tauri/target/release/bundle/msi/guardian_0.1.0_x64.msi.zip.sig` — imza
- `src-tauri/target/release/bundle/msi/latest.json` — update manifest

> Build başarısız olursa hata mesajına göre düzeltme yap.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: updater signing key ve build testi"
```

---

### Task 5: GitHub Secret'ları Ayarlama (Manuel)

Bu task CI/CD'nin çalışması için GitHub repo ayarlarına elle yapılacak işlemleri içerir.

- [ ] **Step 1: Private key'i GitHub Secret'a ekle**

`~/.tauri/guardian-updater.key` dosyasının tüm içeriğini kopyala.

GitHub'da: `Settings > Secrets and variables > Actions` sayfasına git ve şu secret'ları ekle:

| Secret | Değer |
|---|---|
| `TAURI_PRIVATE_KEY` | `guardian-updater.key` dosyasının tam içeriği |
| `TAURI_KEY_PASSWORD` | Key oluşturulurken girilen şifre |

> **ÖNEMLİ:** Private key'i asla repo'ya commit etme. Sadece GitHub Secret olarak sakla.

- [ ] **Step 2: Release tag'i oluşturup pipeline'ı test et**

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions'da workflow'un tetiklendiğini ve başarıyla tamamlandığını doğrula. GitHub Releases sayfasında draft bir release oluşmuş olmalı.

- [ ] **Step 3: Draft release'i yayınla**

GitHub Releases sayfasına git, draft release'i kontrol et, gerekirse release notes'u düzenle ve `Publish release` yap.
