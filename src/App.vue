<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import Swal from "sweetalert2";
import "sweetalert2/dist/sweetalert2.min.css";



interface ProcessInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  status: "Stopped" | "Running" | "Crashed" | "Restarting" | "Stopping";
  restart_count: number;
  pid?: number;
  uptime_secs: number;
}

const processes = ref<ProcessInfo[]>([]);
const selectedProcessId = ref<string | null>(null);
const logs = ref<string[]>([]);
const logTerminals = ref<Record<string, string[]>>({});
const isAddingProcess = ref(false);
const autoScroll = ref(true);
const terminalRef = ref<HTMLElement | null>(null);

const newProcess = ref({
  id: "",
  name: "",
  command: "",
  cwd: "",
  auto_restart: true,
  max_restarts: 5,
  auto_start: true,
});
const argsInput = ref("");

// Stats computed properties
const stats = computed(() => {
  const total = processes.value.length;
  const running = processes.value.filter((p) => p.status === "Running").length;
  const restarting = processes.value.filter((p) => p.status === "Restarting").length;
  const crashed = processes.value.filter((p) => p.status === "Crashed").length;
  const stopped = processes.value.filter((p) => p.status === "Stopped").length;
  return { total, running, restarting, crashed, stopped };
});

const selectedProcess = computed(() => {
  return processes.value.find((p) => p.id === selectedProcessId.value) || null;
});

async function fetchProcesses() {
  try {
    processes.value = await invoke<ProcessInfo[]>("get_processes");
  } catch (error) {
    console.error("Failed to fetch processes:", error);
  }
}

async function browseExecutable() {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Çalıştırılabilir Dosya Seç",
      filters: [
        {
          name: "Çalıştırılabilir Dosyalar",
          extensions: ["exe", "bat", "cmd", "ps1", "vbs", "jar", "sh"]
        },
        {
          name: "Tüm Dosyalar",
          extensions: ["*"]
        }
      ]
    });
    if (selected && typeof selected === "string") {
      newProcess.value.command = selected;
    }
  } catch (error) {
    console.error("Failed to select file:", error);
  }
}

async function selectProcess(id: string) {
  selectedProcessId.value = id;
  try {
    const historicalLogs = await invoke<string[]>("get_process_logs", { id, maxLines: 150 });
    logTerminals.value[id] = historicalLogs;
    logs.value = historicalLogs;
    scrollToBottom();
  } catch (error) {
    console.error("Failed to fetch historical logs:", error);
    logs.value = [`[Sistem Hatası] Loglar alınamadı: ${error}`];
  }
}

function scrollToBottom() {
  if (autoScroll.value) {
    nextTick(() => {
      if (terminalRef.value) {
        terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
      }
    });
  }
}

async function handleStartProcess(id: string) {
  const result = await Swal.fire({
    title: "Süreci Başlat?",
    text: `"${id}" sürecini başlatmak istediğinizden emin misiniz?`,
    icon: "question",
    showCancelButton: true,
    confirmButtonText: "Evet, Başlat",
    cancelButtonText: "İptal",
    background: "#151b2d",
    color: "#f3f4f6",
    confirmButtonColor: "#3b82f6",
    cancelButtonColor: "rgba(255, 255, 255, 0.08)",
  });

  if (result.isConfirmed) {
    try {
      await invoke("start_process", { id });
      await fetchProcesses();
      Swal.fire({
        title: "Başlatıldı!",
        text: "Süreç başarıyla başlatıldı.",
        icon: "success",
        timer: 1500,
        showConfirmButton: false,
        background: "#151b2d",
        color: "#f3f4f6",
      });
    } catch (error) {
      Swal.fire({
        title: "Hata!",
        text: `Süreç başlatılamadı: ${error}`,
        icon: "error",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#3b82f6",
      });
    }
  }
}

async function handleStopProcess(id: string) {
  const result = await Swal.fire({
    title: "Süreci Durdur?",
    text: `"${id}" sürecini durdurmak istediğinizden emin misiniz?`,
    icon: "warning",
    showCancelButton: true,
    confirmButtonText: "Evet, Durdur",
    cancelButtonText: "İptal",
    background: "#151b2d",
    color: "#f3f4f6",
    confirmButtonColor: "#ef4444",
    cancelButtonColor: "rgba(255, 255, 255, 0.08)",
  });

  if (result.isConfirmed) {
    try {
      await invoke("stop_process", { id });
      await fetchProcesses();
      Swal.fire({
        title: "Durduruldu!",
        text: "Süreç başarıyla durduruldu.",
        icon: "success",
        timer: 1500,
        showConfirmButton: false,
        background: "#151b2d",
        color: "#f3f4f6",
      });
    } catch (error) {
      Swal.fire({
        title: "Hata!",
        text: `Süreç durdurulamadı: ${error}`,
        icon: "error",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#3b82f6",
      });
    }
  }
}

async function handleRemoveProcess(id: string) {
  const result = await Swal.fire({
    title: "Emin misiniz?",
    text: "Bu süreci silmek istediğinizden emin misiniz? Bu işlem geri alınamaz.",
    icon: "warning",
    showCancelButton: true,
    confirmButtonText: "Evet, Sil",
    cancelButtonText: "İptal",
    background: "#151b2d",
    color: "#f3f4f6",
    confirmButtonColor: "#ef4444",
    cancelButtonColor: "rgba(255, 255, 255, 0.08)",
  });

  if (result.isConfirmed) {
    try {
      await invoke("remove_process", { id });
      if (selectedProcessId.value === id) {
        selectedProcessId.value = null;
        logs.value = [];
      }
      await fetchProcesses();
      Swal.fire({
        title: "Silindi!",
        text: "Süreç başarıyla kaldırıldı.",
        icon: "success",
        timer: 1500,
        showConfirmButton: false,
        background: "#151b2d",
        color: "#f3f4f6",
      });
    } catch (error) {
      Swal.fire({
        title: "Hata!",
        text: `Kaldırma hatası: ${error}`,
        icon: "error",
        background: "#151b2d",
        color: "#f3f4f6",
        confirmButtonColor: "#3b82f6",
      });
    }
  }
}

async function handleAddProcess() {
  if (!newProcess.value.id || !newProcess.value.name || !newProcess.value.command) {
    Swal.fire({
      title: "Eksik Bilgi!",
      text: "Lütfen gerekli alanları doldurun.",
      icon: "warning",
      background: "#151b2d",
      color: "#f3f4f6",
      confirmButtonColor: "#3b82f6",
    });
    return;
  }

  const rawArgs = argsInput.value.trim();
  const parsedArgs = rawArgs ? rawArgs.split(/\s+/) : [];

  const config = {
    id: newProcess.value.id.trim(),
    name: newProcess.value.name.trim(),
    command: newProcess.value.command.trim(),
    args: parsedArgs,
    cwd: newProcess.value.cwd.trim() || undefined,
    auto_restart: newProcess.value.auto_restart,
    max_restarts: Number(newProcess.value.max_restarts),
    auto_start: newProcess.value.auto_start,
  };

  try {
    await invoke("add_process", { config });
    isAddingProcess.value = false;
    // reset form
    newProcess.value = {
      id: "",
      name: "",
      command: "",
      cwd: "",
      auto_restart: true,
      max_restarts: 5,
      auto_start: true,
    };
    argsInput.value = "";
    await fetchProcesses();
    Swal.fire({
      title: "Başarılı!",
      text: "Yeni süreç başarıyla eklendi.",
      icon: "success",
      timer: 1500,
      showConfirmButton: false,
      background: "#151b2d",
      color: "#f3f4f6",
    });
  } catch (error) {
    Swal.fire({
      title: "Hata!",
      text: `Ekleme hatası: ${error}`,
      icon: "error",
      background: "#151b2d",
      color: "#f3f4f6",
      confirmButtonColor: "#3b82f6",
    });
  }
}

function clearTerminal() {
  if (selectedProcessId.value) {
    logTerminals.value[selectedProcessId.value] = [];
    logs.value = [];
  }
}

function formatUptime(secs: number): string {
  if (secs === 0) return "-";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  
  const parts = [];
  if (h > 0) parts.push(`${h}sa`);
  if (m > 0 || h > 0) parts.push(`${m}dk`);
  parts.push(`${s}sn`);
  return parts.join(" ");
}

let unlistenLog: (() => void) | null = null;
let unlistenStatus: (() => void) | null = null;
let pollInterval: any = null;

onMounted(async () => {
  await fetchProcesses();

  // Listen for log lines from Rust
  unlistenLog = await listen<{ id: string; line: string }>("log-line", (event) => {
    const { id, line } = event.payload;
    if (!logTerminals.value[id]) {
      logTerminals.value[id] = [];
    }
    logTerminals.value[id].push(line);
    if (logTerminals.value[id].length > 500) {
      logTerminals.value[id].shift();
    }

    if (selectedProcessId.value === id) {
      logs.value = logTerminals.value[id];
      scrollToBottom();
    }
  });

  // Listen for status changes from Rust
  unlistenStatus = await listen<{ id: string; status: ProcessInfo["status"] }>(
    "status-changed",
    (event) => {
      const { id, status } = event.payload;
      const process = processes.value.find((p) => p.id === id);
      if (process) {
        process.status = status;
        if (status === "Stopped" || status === "Crashed") {
          process.pid = undefined;
          process.uptime_secs = 0;
        }
      } else {
        fetchProcesses();
      }
    }
  );

  // Poll for uptime updates and active list
  pollInterval = setInterval(() => {
    fetchProcesses();
  }, 2000);
});

onUnmounted(() => {
  if (unlistenLog) unlistenLog();
  if (unlistenStatus) unlistenStatus();
  if (pollInterval) clearInterval(pollInterval);
});
</script>

<template>
  <div class="app-layout">
    <!-- Sidebar / Top Panel -->
    <header class="app-header">
      <div class="brand">
        <div class="logo-shield">🛡️</div>
        <div class="brand-text">
          <h1>GUARDIAN</h1>
          <p>Windows Supervisor Engine</p>
        </div>
      </div>
      <button class="btn btn-primary" @click="isAddingProcess = true">
        <span class="btn-icon">+</span> Yeni Süreç Ekle
      </button>
    </header>

    <!-- Stats Dashboard -->
    <section class="stats-grid">
      <div class="stat-card">
        <span class="stat-value">{{ stats.total }}</span>
        <span class="stat-label">Toplam Süreç</span>
      </div>
      <div class="stat-card stat-running">
        <span class="stat-value">{{ stats.running }}</span>
        <span class="stat-label">Çalışıyor</span>
      </div>
      <div class="stat-card stat-restarting" v-if="stats.restarting > 0">
        <span class="stat-value">{{ stats.restarting }}</span>
        <span class="stat-label">Yeniden Başlıyor</span>
      </div>
      <div class="stat-card stat-crashed">
        <span class="stat-value">{{ stats.crashed }}</span>
        <span class="stat-label">Çöken</span>
      </div>
      <div class="stat-card stat-stopped">
        <span class="stat-value">{{ stats.stopped }}</span>
        <span class="stat-label">Durdurulmuş</span>
      </div>
    </section>

    <!-- Main Workspace -->
    <div class="workspace">
      <!-- Process List -->
      <section class="process-section">
        <h2 class="section-title">Süreçler</h2>
        <div v-if="processes.length === 0" class="empty-state">
          <p>Kayıtlı süreç bulunamadı. Yeni bir süreç ekleyerek başlayın.</p>
        </div>
        <div class="process-list" v-else>
          <div
            v-for="p in processes"
            :key="p.id"
            class="process-card"
            :class="{ active: selectedProcessId === p.id }"
            @click="selectProcess(p.id)"
          >
            <div class="process-card-header">
              <div class="title-area">
                <span class="status-indicator" :class="p.status.toLowerCase()"></span>
                <h3>{{ p.name }}</h3>
                <span class="process-id-badge">{{ p.id }}</span>
              </div>
              <span class="status-badge" :class="p.status.toLowerCase()">{{ p.status }}</span>
            </div>

            <div class="process-details">
              <div class="detail-row">
                <span class="detail-label">Komut:</span>
                <span class="detail-val code-text">{{ p.command }} {{ p.args.join(' ') }}</span>
              </div>
              <div class="detail-metrics">
                <div class="metric">
                  <span class="metric-label">PID</span>
                  <span class="metric-value">{{ p.pid || '-' }}</span>
                </div>
                <div class="metric">
                  <span class="metric-label">Çalışma Süresi</span>
                  <span class="metric-value">{{ formatUptime(p.uptime_secs) }}</span>
                </div>
                <div class="metric">
                  <span class="metric-label">Yeniden Başlama</span>
                  <span class="metric-value">{{ p.restart_count }}</span>
                </div>
              </div>
            </div>

            <div class="process-card-actions" @click.stop>
              <button
                v-if="p.status !== 'Running' && p.status !== 'Restarting'"
                class="btn-action btn-start"
                title="Başlat"
                @click="handleStartProcess(p.id)"
              >
                ▶ Başlat
              </button>
              <button
                v-else
                class="btn-action btn-stop"
                title="Durdur"
                @click="handleStopProcess(p.id)"
              >
                ■ Durdur
              </button>
              <button
                class="btn-action btn-logs"
                :class="{ active: selectedProcessId === p.id }"
                title="Logları Gör"
                @click="selectProcess(p.id)"
              >
                📋 Loglar
              </button>
              <button
                class="btn-action btn-danger"
                title="Kaldır"
                @click="handleRemoveProcess(p.id)"
              >
                🗑️ Sil
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- Log Terminal -->
      <section class="terminal-section">
        <div class="terminal-header">
          <div class="terminal-title">
            <span class="terminal-icon">$_</span>
            <h2>Terminal Logları: {{ selectedProcess ? selectedProcess.name : 'Seçilmedi' }}</h2>
          </div>
          <div class="terminal-controls" v-if="selectedProcessId">
            <label class="control-label">
              <input type="checkbox" v-model="autoScroll" /> Otomatik Kaydır
            </label>
            <button class="btn btn-secondary btn-sm" @click="clearTerminal">Temizle</button>
          </div>
        </div>
        <div class="terminal-body" ref="terminalRef">
          <div v-if="!selectedProcessId" class="terminal-empty">
            <p>Logları incelemek için listeden bir sürece tıklayın.</p>
          </div>
          <div v-else-if="logs.length === 0" class="terminal-empty">
            <p>Süreçten henüz bir çıktı alınmadı veya log dosyası boş.</p>
          </div>
          <div v-else class="log-container">
            <div
              v-for="(line, idx) in logs"
              :key="idx"
              class="log-line"
              :class="{
                'log-err': line.includes('[STDERR]'),
                'log-system': line.includes('[System Error]')
              }"
            >
              {{ line }}
            </div>
          </div>
        </div>
      </section>
    </div>

    <!-- Add Process Modal Overlay -->
    <div class="modal-overlay" v-if="isAddingProcess" @click.self="isAddingProcess = false">
      <div class="modal-card">
        <div class="modal-header">
          <h2>Yeni Süreç Yapılandır</h2>
          <button class="close-btn" @click="isAddingProcess = false">&times;</button>
        </div>
        <form @submit.prevent="handleAddProcess" class="modal-form">
          <div class="form-group-row">
            <div class="form-group">
              <label for="proc-id">Süreç ID (Benzersiz)</label>
              <input
                type="text"
                id="proc-id"
                v-model="newProcess.id"
                placeholder="örn. web-server"
                required
              />
            </div>
            <div class="form-group">
              <label for="proc-name">Süreç Adı</label>
              <input
                type="text"
                id="proc-name"
                v-model="newProcess.name"
                placeholder="örn. Node Web Sunucusu"
                required
              />
            </div>
          </div>

          <div class="form-group">
            <label for="proc-cmd">Çalıştırılabilir Komut / Dosya Yolu</label>
            <div class="input-with-button">
              <input
                type="text"
                id="proc-cmd"
                v-model="newProcess.command"
                placeholder="örn. node veya C:\Program Files\nodejs\node.exe"
                required
              />
              <button type="button" class="btn btn-secondary" @click="browseExecutable">
                Gözat...
              </button>
            </div>
          </div>

          <div class="form-group">
            <label for="proc-args">Argümanlar (Boşluklarla ayrılmış)</label>
            <input
              type="text"
              id="proc-args"
              v-model="argsInput"
              placeholder="örn. index.js --port 3000"
            />
          </div>

          <div class="form-group">
            <label for="proc-cwd">Çalışma Dizini (CWD - Opsiyonel)</label>
            <input
              type="text"
              id="proc-cwd"
              v-model="newProcess.cwd"
              placeholder="örn. C:\Users\Proje"
            />
          </div>

          <div class="checkbox-group">
            <input type="checkbox" id="proc-autostart" v-model="newProcess.auto_start" />
            <label for="proc-autostart">Otomatik Başlat (Uygulama açıldığında otomatik başlar)</label>
          </div>

          <div class="checkbox-group">
            <input type="checkbox" id="proc-restart" v-model="newProcess.auto_restart" />
            <label for="proc-restart">Otomatik Yeniden Başlat (Crash durumunda süreci kurtarır)</label>
          </div>

          <div class="form-group" v-if="newProcess.auto_restart">
            <label for="proc-max-restarts">Maksimum Yeniden Başlatma Limiti</label>
            <input
              type="number"
              id="proc-max-restarts"
              v-model="newProcess.max_restarts"
              min="1"
              max="50"
            />
          </div>

          <div class="modal-actions">
            <button type="button" class="btn btn-secondary" @click="isAddingProcess = false">
              İptal
            </button>
            <button type="submit" class="btn btn-primary">Kaydet ve Ekle</button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap');

:root {
  --bg-app: #0b0f19;
  --bg-card: #151b2d;
  --bg-input: #1e2640;
  --bg-terminal: #070a13;
  --text-main: #f3f4f6;
  --text-muted: #9ca3af;
  --accent-primary: #3b82f6;
  --accent-primary-hover: #2563eb;
  --status-running: #10b981;
  --status-stopped: #6b7280;
  --status-crashed: #ef4444;
  --status-restarting: #f59e0b;
  --status-stopping: #8b5cf6;
  --border-color: rgba(255, 255, 255, 0.08);
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  background-color: var(--bg-app);
  color: var(--text-main);
  overflow: hidden;
  height: 100vh;
}

#app {
  height: 100%;
}

.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 1.5rem;
  gap: 1.5rem;
}

/* Header */
.app-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 1rem;
}

.brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.logo-shield {
  font-size: 2.25rem;
}

.brand-text h1 {
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: 2px;
  background: linear-gradient(135deg, #60a5fa, #3b82f6);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.brand-text p {
  font-size: 0.8rem;
  color: var(--text-muted);
}

/* Stats */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 1rem;
}

.stat-card {
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
  position: relative;
  overflow: hidden;
}

.stat-card::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  background: var(--accent-primary);
}

.stat-card.stat-running::before { background: var(--status-running); }
.stat-card.stat-restarting::before { background: var(--status-restarting); }
.stat-card.stat-crashed::before { background: var(--status-crashed); }
.stat-card.stat-stopped::before { background: var(--status-stopped); }

.stat-value {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1.2;
}

.stat-label {
  font-size: 0.85rem;
  color: var(--text-muted);
}

/* Main Workspace */
.workspace {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  flex: 1;
  min-height: 0; /* Important for scroll limits */
}

.process-section {
  flex: 1;
}

.terminal-section {
  flex: 1.2;
}

.process-section, .terminal-section {
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  min-height: 0;
  box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.2);
}

.section-title {
  font-size: 1.1rem;
  font-weight: 600;
  padding: 1.25rem;
  border-bottom: 1px solid var(--border-color);
}

.empty-state {
  display: flex;
  justify-content: center;
  align-items: center;
  flex: 1;
  color: var(--text-muted);
  text-align: center;
  padding: 2rem;
}

.process-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 400px));
  gap: 1rem;
  padding: 1.25rem;
  overflow-y: auto;
  flex: 1;
}

/* Process Card */
.process-card {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 1rem;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.process-card:hover {
  background: rgba(255, 255, 255, 0.04);
  transform: translateY(-2px);
  border-color: rgba(59, 130, 246, 0.4);
}

.process-card.active {
  background: rgba(59, 130, 246, 0.08);
  border-color: var(--accent-primary);
}

.process-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title-area {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.status-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  box-shadow: 0 0 8px currentColor;
}

.status-indicator.running { color: var(--status-running); background: var(--status-running); }
.status-indicator.stopped { color: var(--status-stopped); background: var(--status-stopped); }
.status-indicator.crashed { color: var(--status-crashed); background: var(--status-crashed); }
.status-indicator.restarting { color: var(--status-restarting); background: var(--status-restarting); }
.status-indicator.stopping { color: var(--status-stopping); background: var(--status-stopping); }

.process-card h3 {
  font-size: 0.95rem;
  font-weight: 600;
}

.process-id-badge {
  font-size: 0.7rem;
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-muted);
  padding: 0.1rem 0.4rem;
  border-radius: 4px;
}

.status-badge {
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.2rem 0.5rem;
  border-radius: 20px;
  text-transform: uppercase;
}

.status-badge.running { background: rgba(16, 185, 129, 0.15); color: var(--status-running); }
.status-badge.stopped { background: rgba(107, 114, 128, 0.15); color: var(--status-stopped); }
.status-badge.crashed { background: rgba(239, 68, 68, 0.15); color: var(--status-crashed); }
.status-badge.restarting { background: rgba(245, 158, 171, 0.15); color: var(--status-restarting); }
.status-badge.stopping { background: rgba(139, 92, 246, 0.15); color: var(--status-stopping); }

.process-details {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.detail-row {
  display: flex;
  font-size: 0.8rem;
  gap: 0.25rem;
}

.detail-label {
  color: var(--text-muted);
  flex-shrink: 0;
}

.detail-val {
  word-break: break-all;
}

.code-text {
  font-family: 'JetBrains Mono', monospace;
  font-size: 0.75rem;
  background: rgba(0, 0, 0, 0.2);
  padding: 0.1rem 0.3rem;
  border-radius: 4px;
}

.detail-metrics {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.5rem;
  background: rgba(0, 0, 0, 0.1);
  padding: 0.5rem;
  border-radius: 8px;
  text-align: center;
}

.metric {
  display: flex;
  flex-direction: column;
}

.metric-label {
  font-size: 0.65rem;
  color: var(--text-muted);
}

.metric-value {
  font-size: 0.8rem;
  font-weight: 600;
}

.process-card-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.25rem;
}

/* Buttons */
.btn {
  border-radius: 8px;
  font-weight: 500;
  padding: 0.5rem 1rem;
  font-size: 0.9rem;
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s ease;
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.btn-primary {
  background: var(--accent-primary);
  color: white;
}

.btn-primary:hover {
  background: var(--accent-primary-hover);
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-main);
  border-color: var(--border-color);
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.12);
}

.btn-sm {
  font-size: 0.75rem;
  padding: 0.25rem 0.5rem;
}

.btn-action {
  border: none;
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-main);
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-action:hover {
  background: rgba(255, 255, 255, 0.1);
}

.btn-start {
  color: var(--status-running);
  background: rgba(16, 185, 129, 0.1);
}
.btn-start:hover { background: rgba(16, 185, 129, 0.2); }

.btn-stop {
  color: var(--status-crashed);
  background: rgba(239, 68, 68, 0.1);
}
.btn-stop:hover { background: rgba(239, 68, 68, 0.2); }

.btn-logs.active {
  color: var(--accent-primary);
  background: rgba(59, 130, 246, 0.15);
  border: 1px solid rgba(59, 130, 246, 0.3);
}

.btn-danger {
  color: var(--status-crashed);
}
.btn-danger:hover {
  background: rgba(239, 68, 68, 0.15);
}

/* Terminal Log Section */
.terminal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.25rem;
  border-bottom: 1px solid var(--border-color);
}

.terminal-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.terminal-icon {
  font-family: 'JetBrains Mono', monospace;
  font-weight: bold;
  color: var(--status-running);
}

.terminal-controls {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.control-label {
  font-size: 0.8rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 0.25rem;
  cursor: pointer;
}

.terminal-body {
  background: var(--bg-terminal);
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
  font-family: 'JetBrains Mono', monospace;
  font-size: 0.8rem;
  line-height: 1.4;
  color: #a7f3d0;
  border-bottom-left-radius: 15px;
  border-bottom-right-radius: 15px;
}

.terminal-empty {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: var(--text-muted);
  font-family: 'Inter', sans-serif;
}

.log-container {
  display: flex;
  flex-direction: column;
}

.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  border-bottom: 1px solid rgba(255, 255, 255, 0.02);
  padding: 0.15rem 0;
}

.log-err {
  color: #fca5a5;
}

.log-system {
  color: #fbbf24;
  font-weight: 500;
}

/* Modal Overlay */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 100;
}

.modal-card {
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  width: 550px;
  max-width: 90%;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
  animation: slideUp 0.2s ease-out;
}

@keyframes slideUp {
  from { transform: translateY(20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.25rem 1.5rem;
  border-bottom: 1px solid var(--border-color);
}

.close-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 1.5rem;
  cursor: pointer;
}

.close-btn:hover {
  color: var(--text-main);
}

.modal-form {
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-group-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.form-group label {
  font-size: 0.8rem;
  color: var(--text-muted);
  font-weight: 500;
}

.form-group input[type="text"],
.form-group input[type="number"] {
  background: var(--bg-input);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 0.6rem 0.8rem;
  color: var(--text-main);
  outline: none;
  font-family: inherit;
  font-size: 0.9rem;
  transition: border-color 0.15s ease;
}

.form-group input:focus {
  border-color: var(--accent-primary);
}

.checkbox-group {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid var(--border-color);
  padding: 0.75rem 1rem;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.checkbox-group:hover {
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(59, 130, 246, 0.3);
}

.checkbox-group input[type="checkbox"] {
  width: 16px;
  height: 16px;
  cursor: pointer;
}

.checkbox-group label {
  font-size: 0.85rem;
  color: var(--text-main);
  cursor: pointer;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 1rem;
  border-top: 1px solid var(--border-color);
  padding-top: 1.25rem;
}

.input-with-button {
  display: flex;
  gap: 0.5rem;
  align-items: stretch;
}

.input-with-button input,
.input-with-button .btn {
  height: 40px;
}

.input-with-button input {
  flex: 1;
}

.input-with-button .btn {
  padding: 0 1.25rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* SweetAlert2 Custom Styling */
.swal2-popup {
  border-radius: 16px !important;
  border: 1px solid var(--border-color) !important;
  font-family: inherit !important;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5) !important;
}

.swal2-title {
  font-weight: 600 !important;
  font-size: 1.25rem !important;
}

.swal2-html-container {
  font-size: 0.9rem !important;
  color: var(--text-muted) !important;
}

.swal2-actions {
  gap: 0.5rem !important;
}

.swal2-confirm, .swal2-cancel {
  border-radius: 8px !important;
  font-weight: 500 !important;
  font-size: 0.9rem !important;
  padding: 0.6rem 1.25rem !important;
  outline: none !important;
}

.swal2-confirm:focus, .swal2-cancel:focus {
  box-shadow: none !important;
}
</style>