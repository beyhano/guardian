<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import Swal from 'sweetalert2'

const checking = ref(false)
let autoCheckTimer: ReturnType<typeof setTimeout> | null = null

async function promptAndInstall(update: Awaited<ReturnType<typeof check>>) {
  if (!update?.available) return

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
    Swal.fire({
      title: 'Güncelleme İndiriliyor...',
      html: '<div style="text-align:center;padding:1rem"><div style="width:28px;height:28px;border:3px solid rgba(108,92,231,0.3);border-top-color:#6c5ce7;border-radius:50%;animation:update-spin 0.6s linear infinite;margin:0 auto"></div></div><style>@keyframes update-spin{to{transform:rotate(360deg)}}</style>',
      allowOutsideClick: false,
      showConfirmButton: false,
      background: '#1a1a2e',
      color: '#eee',
    })

    await update.downloadAndInstall()
    await relaunch()
  }
}

async function checkForUpdates(silent = false) {
  if (checking.value) return
  checking.value = true

  try {
    const update = await check()
    if (update?.available) {
      await promptAndInstall(update)
    } else if (!silent) {
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
    if (!silent) {
      await Swal.fire({
        title: 'Kontrol Başarısız',
        text: `Güncelleme kontrolü yapılamadı: ${err}`,
        icon: 'error',
        confirmButtonColor: '#6c5ce7',
        background: '#1a1a2e',
        color: '#eee',
      })
    }
  } finally {
    checking.value = false
  }
}

onMounted(() => {
  autoCheckTimer = setTimeout(() => checkForUpdates(true), 3000)
})

onUnmounted(() => {
  if (autoCheckTimer) clearTimeout(autoCheckTimer)
})
</script>

<template>
  <button
    class="update-btn"
    :disabled="checking"
    @click="checkForUpdates()"
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
