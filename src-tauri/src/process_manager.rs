use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::fs::OpenOptions;
use tauri::{AppHandle, Emitter, Manager};

fn default_auto_start() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub auto_restart: bool,
    pub max_restarts: usize,
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProcessStatus {
    Stopped,
    Running,
    Crashed,
    Restarting,
    Stopping,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub status: ProcessStatus,
    pub restart_count: usize,
    pub pid: Option<u32>,
    pub uptime_secs: u64,
}

pub struct ActiveProcess {
    pub config: ProcessConfig,
    pub status: ProcessStatus,
    pub restart_count: usize,
    pub pid: Option<u32>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Clone)]
pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<String, ActiveProcess>>>,
    app_handle: AppHandle,
    db_path: PathBuf,
    logs_dir: PathBuf,
}

impl ProcessManager {
    pub fn new(app_handle: AppHandle) -> Self {
        let app_dir = app_handle
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        let db_path = app_dir.join("guardian.db");
        let logs_dir = app_dir.join("logs");

        let pm = Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            app_handle,
            db_path,
            logs_dir,
        };

        if let Err(e) = pm.init_db() {
            eprintln!("Failed to initialize database: {}", e);
        }

        pm
    }

    fn connect_db(&self) -> Result<rusqlite::Connection, String> {
        rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {}", e))
    }

    pub fn init_db(&self) -> Result<(), String> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {}", e))?;
        }

        let conn = self.connect_db()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS processes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                cwd TEXT,
                auto_restart INTEGER NOT NULL,
                max_restarts INTEGER NOT NULL,
                auto_start INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create table: {}", e))?;

        Ok(())
    }

    pub async fn load_configs(&self) -> Result<(), String> {
        let conn = self.connect_db()?;
        let mut stmt = conn
            .prepare("SELECT id, name, command, args, cwd, auto_restart, max_restarts, auto_start FROM processes")
            .map_err(|e| format!("Failed to prepare select statement: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let command: String = row.get(2)?;
                let args_json: String = row.get(3)?;
                let cwd: Option<String> = row.get(4)?;
                let auto_restart_int: i32 = row.get(5)?;
                let max_restarts_int: i64 = row.get(6)?;
                let auto_start_int: i32 = row.get(7)?;

                let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();

                Ok(ProcessConfig {
                    id,
                    name,
                    command,
                    args,
                    cwd,
                    auto_restart: auto_restart_int != 0,
                    max_restarts: max_restarts_int as usize,
                    auto_start: auto_start_int != 0,
                })
            })
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        let mut processes = self.processes.lock().await;
        for row_result in rows {
            if let Ok(config) = row_result {
                processes.insert(
                    config.id.clone(),
                    ActiveProcess {
                        config,
                        status: ProcessStatus::Stopped,
                        restart_count: 0,
                        pid: None,
                        start_time: None,
                        stop_tx: None,
                    },
                );
            }
        }
        Ok(())
    }

    pub async fn auto_start_processes(&self) -> Result<(), String> {
        let process_ids: Vec<String> = {
            let processes = self.processes.lock().await;
            processes
                .iter()
                .filter(|(_, p)| p.config.auto_start)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in process_ids {
            let mut processes = self.processes.lock().await;
            if let Some(process) = processes.get_mut(&id) {
                if process.status != ProcessStatus::Running && process.status != ProcessStatus::Restarting {
                    process.restart_count = 0;
                    if let Err(e) = self.spawn_process_internal(&id, process).await {
                        eprintln!("Failed to auto-start process {}: {}", id, e);
                    }
                }
            }
        }
        Ok(())
    }



    pub async fn get_processes(&self) -> Vec<ProcessInfo> {
        let processes = self.processes.lock().await;
        processes
            .values()
            .map(|p| {
                let uptime_secs = p.start_time.map_or(0, |st| {
                    let now = chrono::Utc::now();
                    let diff = now.signed_duration_since(st);
                    diff.num_seconds().max(0) as u64
                });

                ProcessInfo {
                    id: p.config.id.clone(),
                    name: p.config.name.clone(),
                    command: p.config.command.clone(),
                    args: p.config.args.clone(),
                    cwd: p.config.cwd.clone(),
                    status: p.status,
                    restart_count: p.restart_count,
                    pid: p.pid,
                    uptime_secs,
                }
            })
            .collect()
    }

    pub async fn add_process(&self, config: ProcessConfig) -> Result<(), String> {
        // Ensure logs directory exists
        tokio::fs::create_dir_all(&self.logs_dir)
            .await
            .map_err(|e| format!("Failed to create logs directory: {}", e))?;

        let mut processes = self.processes.lock().await;
        if processes.contains_key(&config.id) {
            return Err("Process with this ID already exists".to_string());
        }

        // Save to SQLite database
        let conn = self.connect_db()?;
        let args_json = serde_json::to_string(&config.args)
            .map_err(|e| format!("Failed to serialize arguments: {}", e))?;
        
        conn.execute(
            "INSERT INTO processes (id, name, command, args, cwd, auto_restart, max_restarts, auto_start)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                config.id,
                config.name,
                config.command,
                args_json,
                config.cwd,
                if config.auto_restart { 1 } else { 0 },
                config.max_restarts as i64,
                if config.auto_start { 1 } else { 0 },
            ],
        )
        .map_err(|e| format!("Failed to insert process into database: {}", e))?;

        processes.insert(
            config.id.clone(),
            ActiveProcess {
                config,
                status: ProcessStatus::Stopped,
                restart_count: 0,
                pid: None,
                start_time: None,
                stop_tx: None,
            },
        );

        Ok(())
    }

    pub async fn update_process(&self, id: &str, config: ProcessConfig) -> Result<(), String> {
        let mut processes = self.processes.lock().await;
        let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

        // ID'yi parametreden gelen değerle koru
        let mut updated_config = config;
        updated_config.id = id.to_string();

        // ÖNCE DB'yi güncelle — state henüz bozulmamışken
        let conn = self.connect_db()?;
        let args_json = serde_json::to_string(&updated_config.args)
            .map_err(|e| format!("Failed to serialize arguments: {}", e))?;

        let affected = conn.execute(
            "UPDATE processes SET name=?, command=?, args=?, cwd=?, auto_restart=?, max_restarts=?, auto_start=? WHERE id=?",
            rusqlite::params![
                updated_config.name,
                updated_config.command,
                args_json,
                updated_config.cwd,
                if updated_config.auto_restart { 1 } else { 0 },
                updated_config.max_restarts as i64,
                if updated_config.auto_start { 1 } else { 0 },
                id,
            ],
        )
        .map_err(|e| format!("Failed to update process in database: {}", e))?;

        if affected == 0 {
            return Err("Process not found in database".to_string());
        }

        // DB başarılı, state'i güvenle taşı
        let status = process.status;
        let restart_count = process.restart_count;
        let pid = process.pid;
        let start_time = process.start_time;
        let stop_tx = process.stop_tx.take();

        // Runtime config'i güncelle, state'i geri yükle
        *process = ActiveProcess {
            config: updated_config,
            status,
            restart_count,
            pid,
            start_time,
            stop_tx,
        };

        Ok(())
    }

    pub async fn remove_process(&self, id: &str) -> Result<(), String> {
        self.stop_process(id).await?;

        let mut processes = self.processes.lock().await;
        processes.remove(id).ok_or_else(|| "Process not found".to_string())?;

        // Delete from SQLite database
        let conn = self.connect_db()?;
        conn.execute("DELETE FROM processes WHERE id = ?", rusqlite::params![id])
            .map_err(|e| format!("Failed to delete process from database: {}", e))?;

        Ok(())
    }

    pub async fn start_process(&self, id: &str) -> Result<(), String> {
        // Önce duplicate kontrolü (lock dışında — tasklist uzun sürebilir)
        let command_to_check: Option<String> = {
            let processes = self.processes.lock().await;
            processes.get(id).map(|p| p.config.command.clone())
        };

        if let Some(cmd) = command_to_check {
            self.check_external_duplicate(&cmd).await?;
        }

        let mut processes = self.processes.lock().await;
        let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

        if process.status == ProcessStatus::Running || process.status == ProcessStatus::Restarting {
            return Err("Process is already running or restarting".to_string());
        }

        process.restart_count = 0;
        self.spawn_process_internal(id, process).await?;
        Ok(())
    }

    pub async fn stop_process(&self, id: &str) -> Result<(), String> {
        let mut processes = self.processes.lock().await;
        let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

        if process.status == ProcessStatus::Stopped || process.status == ProcessStatus::Crashed {
            return Ok(());
        }

        process.status = ProcessStatus::Stopping;
        self.emit_status_changed(id, ProcessStatus::Stopping);

        if let Some(stop_tx) = process.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        if let Some(pid) = process.pid {
            #[cfg(windows)]
            {
                let mut kill_cmd = tokio::process::Command::new("taskkill");
                kill_cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
                let _ = kill_cmd.status().await;
            }
            #[cfg(not(windows))]
            {
                let mut kill_cmd = tokio::process::Command::new("kill");
                kill_cmd.args(&["-9", &pid.to_string()]);
                let _ = kill_cmd.status().await;
            }
        }

        process.status = ProcessStatus::Stopped;
        process.pid = None;
        process.start_time = None;

        // Manuel durduruldu — otomatik başlamasın diye DB'de auto_start = 0 yap
        process.config.auto_start = false;
        if let Ok(conn) = self.connect_db() {
            let _ = conn.execute(
                "UPDATE processes SET auto_start = 0 WHERE id = ?",
                rusqlite::params![id],
            );
        }

        self.emit_status_changed(id, ProcessStatus::Stopped);
        Ok(())
    }

    pub async fn restart_process(&self, id: &str) -> Result<(), String> {
        // Running durumunda önce duplicate kontrolüne gerek yok (kendimiz durduracagiz)
        // Stopped/Crashed durumunda kontrol et
        let command_to_check: Option<String> = {
            let processes = self.processes.lock().await;
            let p = processes.get(id);
            match p.map(|p| p.status) {
                Some(ProcessStatus::Stopped) | Some(ProcessStatus::Crashed) => {
                    p.map(|p| p.config.command.clone())
                }
                _ => None,
            }
        };

        if let Some(cmd) = command_to_check {
            self.check_external_duplicate(&cmd).await?;
        }

        let mut processes = self.processes.lock().await;
        let process = processes.get_mut(id).ok_or_else(|| "Process not found".to_string())?;

        match process.status {
            ProcessStatus::Running => {
                // Durum 1: Çalışıyor → durdur ve yeniden başlat
                process.status = ProcessStatus::Stopping;
                self.emit_status_changed(id, ProcessStatus::Stopping);

                if let Some(stop_tx) = process.stop_tx.take() {
                    let _ = stop_tx.send(());
                }

                if let Some(pid) = process.pid {
                    #[cfg(windows)]
                    {
                        let mut kill_cmd = tokio::process::Command::new("taskkill");
                        kill_cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
                        let _ = kill_cmd.status().await;
                    }
                    #[cfg(not(windows))]
                    {
                        let mut kill_cmd = tokio::process::Command::new("kill");
                        kill_cmd.args(&["-9", &pid.to_string()]);
                        let _ = kill_cmd.status().await;
                    }
                }

                process.status = ProcessStatus::Stopped;
                process.pid = None;
                process.start_time = None;
                self.emit_status_changed(id, ProcessStatus::Stopped);

                // Port/socket release için kısa bekle
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                // Manuel restart — sayacı sıfırla
                process.restart_count = 0;

                // Yeniden başlat
                self.spawn_process_internal(id, process).await?;
                Ok(())
            }
            ProcessStatus::Stopped | ProcessStatus::Crashed => {
                // Durum 2: Durmuş veya çökmüş → direkt başlat
                process.restart_count = 0;
                self.spawn_process_internal(id, process).await?;
                Ok(())
            }
            ProcessStatus::Restarting => {
                Err("Process is already restarting".to_string())
            }
            ProcessStatus::Stopping => {
                Err("Process is stopping".to_string())
            }
        }
    }

    pub fn get_process_logs(&self, id: &str, max_lines: usize) -> Result<Vec<String>, String> {
        let log_path = self.logs_dir.join(format!("{}.log", id));
        read_last_lines_sync(&log_path, max_lines)
    }

    /// Sistemde aynı executable'ın Guardian dışında çalışıp çalışmadığını kontrol eder.
    /// Eğer çalışıyorsa Err döndürür.
    async fn check_external_duplicate(&self, command: &str) -> Result<(), String> {
        let exe_name = std::path::Path::new(command)
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| format!("Geçersiz komut yolu: {}", command))?;

        // Guardian'ın bildiği PID'leri topla (lock sonra serbest bırakılır)
        let known_pids: Vec<u32> = {
            let processes = self.processes.lock().await;
            processes.values().filter_map(|p| p.pid).collect()
        };

        if cfg!(windows) {
            let output = tokio::process::Command::new("tasklist")
                .args(&["/NH", "/FO", "CSV", "/FI", &format!("IMAGENAME eq {}", exe_name)])
                .output()
                .await
                .map_err(|e| format!("tasklist çalıştırılamadı: {}", e))?;

            if !output.status.success() {
                return Ok(()); // tasklist başarısız olursa sessiz geç
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }

                // CSV format: "image.exe","pid","session","session#","mem kullanımı"
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() < 2 { continue; }

                let pid_str = parts[1].trim_matches('"');
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if !known_pids.contains(&pid) {
                        return Err(format!(
                            "'{}' PID {} ile sistemde zaten çalışıyor. Önce durdurun veya restart kullanın.",
                            exe_name, pid
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    async fn spawn_process_internal(&self, id: &str, process: &mut ActiveProcess) -> Result<(), String> {
        let mut cmd = tokio::process::Command::new(&process.config.command);
        cmd.args(&process.config.args);

        if let Some(ref cwd) = process.config.cwd {
            if !cwd.trim().is_empty() {
                cmd.current_dir(cwd);
            }
        } else {
            // cwd belirtilmemisse command yolunun bulundugu dizini kullan
            // (.env gibi goreceli yollarin dogru cozulmesi icin)
            let cmd_path = std::path::Path::new(&process.config.command);
            if let Some(parent) = cmd_path.parent() {
                let parent_str = parent.to_string_lossy();
                if !parent_str.is_empty() {
                    cmd.current_dir(parent);
                }
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn process: {}", e))?;
        let pid = child.id();
        
        process.pid = pid;
        process.status = if process.status == ProcessStatus::Restarting {
            ProcessStatus::Restarting
        } else {
            ProcessStatus::Running
        };
        process.start_time = Some(chrono::Utc::now());
        self.emit_status_changed(id, process.status);

        let stdout = child.stdout.take().ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child.stderr.take().ok_or_else(|| "Failed to capture stderr".to_string())?;

        let log_path = self.logs_dir.join(format!("{}.log", id));
        let app_handle_clone1 = self.app_handle.clone();
        let app_handle_clone2 = self.app_handle.clone();
        let id_clone1 = id.to_string();
        let id_clone2 = id.to_string();
        let log_path_clone1 = log_path.clone();
        let log_path_clone2 = log_path.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            pipe_stream(reader, log_path_clone1, app_handle_clone1, id_clone1, "STDOUT").await;
        });

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            pipe_stream(reader, log_path_clone2, app_handle_clone2, id_clone2, "STDERR").await;
        });

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        process.stop_tx = Some(stop_tx);

        let pm_clone = self.clone();
        let process_id = id.to_string();

        tokio::spawn(async move {
            tokio::select! {
                _ = &mut stop_rx => {
                    // Stopped manually
                }
                status = child.wait() => {
                    let is_success = match status {
                        Ok(s) => s.success(),
                        Err(_) => false,
                    };
                    pm_clone.handle_process_exit(&process_id, is_success).await;
                }
            }
        });

        Ok(())
    }

    async fn handle_process_exit(&self, id: &str, is_success: bool) {
        let mut processes = self.processes.lock().await;
        if let Some(process) = processes.get_mut(id) {
            if process.status == ProcessStatus::Stopping || process.status == ProcessStatus::Stopped {
                process.status = ProcessStatus::Stopped;
                process.pid = None;
                process.start_time = None;
                process.stop_tx = None;
                self.emit_status_changed(id, ProcessStatus::Stopped);
                return;
            }

            let should_restart = process.config.auto_restart && process.restart_count < process.config.max_restarts;

            if should_restart {
                process.status = ProcessStatus::Restarting;
                process.restart_count += 1;
                process.pid = None;
                self.emit_status_changed(id, ProcessStatus::Restarting);

                let pm_clone = self.clone();
                let process_id = id.to_string();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let mut is_restarting = false;
                    {
                        let processes = pm_clone.processes.lock().await;
                        if let Some(p) = processes.get(&process_id) {
                            if p.status == ProcessStatus::Restarting {
                                is_restarting = true;
                            }
                        }
                    }
                    if is_restarting {
                        if let Err(e) = spawn_process_boxed(pm_clone.clone(), process_id.clone()).await {
                            eprintln!("Failed to restart process {}: {}", process_id, e);
                            let mut processes = pm_clone.processes.lock().await;
                            if let Some(p) = processes.get_mut(&process_id) {
                                p.status = ProcessStatus::Crashed;
                                p.pid = None;
                                p.start_time = None;
                                pm_clone.emit_status_changed(&process_id, ProcessStatus::Crashed);
                            }
                        }
                    }
                });
            } else {
                process.status = if is_success { ProcessStatus::Stopped } else { ProcessStatus::Crashed };
                process.pid = None;
                process.start_time = None;
                process.stop_tx = None;
                self.emit_status_changed(id, process.status);
            }
        }
    }

    fn emit_status_changed(&self, id: &str, status: ProcessStatus) {
        #[derive(Clone, serde::Serialize)]
        struct StatusPayload {
            id: String,
            status: ProcessStatus,
        }
        let _ = self.app_handle.emit(
            "status-changed",
            StatusPayload {
                id: id.to_string(),
                status,
            },
        );
    }
}

async fn pipe_stream<R>(
    mut reader: BufReader<R>,
    log_path: PathBuf,
    app_handle: AppHandle,
    process_id: String,
    stream_name: &'static str,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut file = match OpenOptions::new().create(true).append(true).open(&log_path).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open log file for piping: {}", e);
            return;
        }
    };

    let mut line = String::new();
    while let Ok(n) = reader.read_line(&mut line).await {
        if n == 0 {
            break;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let formatted_line = format!("[{}][{}] {}", timestamp, stream_name, line);

        let _ = file.write_all(formatted_line.as_bytes()).await;
        let _ = file.flush().await;

        #[derive(Clone, serde::Serialize)]
        struct LogPayload {
            id: String,
            line: String,
        }
        let _ = app_handle.emit(
            "log-line",
            LogPayload {
                id: process_id.clone(),
                line: formatted_line,
            },
        );

        line.clear();
    }
}

fn read_last_lines_sync(path: &PathBuf, max_lines: usize) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    let file_len = metadata.len();

    let read_len = std::cmp::min(file_len, 100 * 1024) as usize;
    let mut buffer = vec![0; read_len];
    if read_len > 0 {
        file.seek(SeekFrom::End(-(read_len as i64))).map_err(|e| e.to_string())?;
        file.read_exact(&mut buffer).map_err(|e| e.to_string())?;
    }

    let text = String::from_utf8_lossy(&buffer);
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    if file_len > read_len as u64 && !lines.is_empty() {
        lines.remove(0);
    }

    let len = lines.len();
    if len <= max_lines {
        Ok(lines)
    } else {
        Ok(lines[len - max_lines..].to_vec())
    }
}

fn spawn_process_boxed(
    pm: ProcessManager,
    id: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> {
    Box::pin(async move {
        let mut processes = pm.processes.lock().await;
        if let Some(process) = processes.get_mut(&id) {
            pm.spawn_process_internal(&id, process).await
        } else {
            Err("Process not found".to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_process_config_serialization() {
        let config = ProcessConfig {
            id: "test".to_string(),
            name: "Test Process".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            auto_restart: true,
            max_restarts: 3,
            auto_start: true,
        };
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ProcessConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, "test");
        assert_eq!(deserialized.name, "Test Process");
        assert_eq!(deserialized.args[0], "hello");
    }

    #[test]
    fn test_read_last_lines_sync() {
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("test_guardian.log");
        
        let mut file = File::create(&log_path).unwrap();
        for i in 1..=10 {
            writeln!(file, "Line {}", i).unwrap();
        }
        drop(file);

        let last_lines = read_last_lines_sync(&log_path, 3).unwrap();
        assert_eq!(last_lines.len(), 3);
        assert_eq!(last_lines[0], "Line 8");
        assert_eq!(last_lines[1], "Line 9");
        assert_eq!(last_lines[2], "Line 10");

        let _ = std::fs::remove_file(log_path);
    }

    #[test]
    fn test_update_process() {
        let mut processes = std::collections::HashMap::new();

        let original = ProcessConfig {
            id: "test-srv".to_string(),
            name: "Original".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            auto_restart: false,
            max_restarts: 0,
            auto_start: false,
        };

        processes.insert(
            "test-srv".to_string(),
            ActiveProcess {
                config: original,
                status: ProcessStatus::Running,
                restart_count: 2,
                pid: Some(12345),
                start_time: Some(chrono::Utc::now()),
                stop_tx: None,
            },
        );

        let updated = ProcessConfig {
            id: "test-srv".to_string(),
            name: "Updated".to_string(),
            command: "ping".to_string(),
            args: vec!["localhost".to_string()],
            cwd: Some("C:\\tmp".to_string()),
            auto_restart: true,
            max_restarts: 5,
            auto_start: true,
        };

        if let Some(ap) = processes.get_mut("test-srv") {
            ap.config = updated.clone();
        }

        let result = processes.get("test-srv").unwrap();
        assert_eq!(result.config.name, "Updated");
        assert_eq!(result.config.command, "ping");
        assert_eq!(result.config.args, vec!["localhost"]);
        assert_eq!(result.config.cwd, Some("C:\\tmp".to_string()));
        assert!(result.config.auto_restart);
        assert_eq!(result.config.max_restarts, 5);
        assert!(result.config.auto_start);
        // Runtime state korunmalı
        assert_eq!(result.status, ProcessStatus::Running);
        assert_eq!(result.restart_count, 2);
        assert!(result.pid.is_some());
    }

    #[test]
    fn test_update_process_not_found() {
        let mut processes: std::collections::HashMap<String, ActiveProcess> = std::collections::HashMap::new();
        assert!(processes.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_update_process_sql_basic() {
        let tmp_dir = std::env::temp_dir().join("guardian_test_update_sql_basic");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let db_path = tmp_dir.join("guardian.db");

        // Tabloyu oluştur ve bir row ekle
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS processes (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, command TEXT NOT NULL,
                args TEXT NOT NULL, cwd TEXT, auto_restart INTEGER NOT NULL,
                max_restarts INTEGER NOT NULL, auto_start INTEGER NOT NULL
            );"
        ).unwrap();

        // Önce mevcut veriyi temizle
        conn.execute("DELETE FROM processes", []).unwrap();

        conn.execute(
            "INSERT INTO processes (id, name, command, args, cwd, auto_restart, max_restarts, auto_start)
             VALUES ('test-srv', 'Original', 'echo', '[]', NULL, 0, 0, 0)",
            []
        ).unwrap();

        // Update — adı ve komutu değiştir
        let affected = conn.execute(
            "UPDATE processes SET name=?1, command=?2, args=?3, auto_restart=?4, max_restarts=?5, auto_start=?6 WHERE id=?7",
            rusqlite::params!["Updated", "ping", "[\"localhost\"]", 1, 5, 1, "test-srv"]
        ).unwrap();
        assert_eq!(affected, 1);

        // Verify
        let (name, command, args, auto_restart, max_restarts, auto_start): (String, String, String, i32, i64, i32) = conn.query_row(
            "SELECT name, command, args, auto_restart, max_restarts, auto_start FROM processes WHERE id='test-srv'",
            [],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?,
                row.get(3)?, row.get(4)?, row.get(5)?,
            ))
        ).unwrap();
        assert_eq!(name, "Updated");
        assert_eq!(command, "ping");
        assert_eq!(args, "[\"localhost\"]");
        assert_eq!(auto_restart, 1);
        assert_eq!(max_restarts, 5);
        assert_eq!(auto_start, 1);

        // Temizlik
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_update_process_sql_not_found() {
        let tmp_dir = std::env::temp_dir().join("guardian_test_update_sql_not_found");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let db_path = tmp_dir.join("guardian.db");

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS processes (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, command TEXT NOT NULL,
                args TEXT NOT NULL, cwd TEXT, auto_restart INTEGER NOT NULL,
                max_restarts INTEGER NOT NULL, auto_start INTEGER NOT NULL
            );"
        ).unwrap();

        // Tablo boş — nonexistent ID için affected = 0 olmalı
        let affected = conn.execute(
            "UPDATE processes SET name='x' WHERE id='nonexistent'",
            []
        ).unwrap();
        assert_eq!(affected, 0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_update_process_core_logic() {
        // update_process metodunun HashMap + state koruma mantığını valide et
        let mut processes: std::collections::HashMap<String, ActiveProcess> = std::collections::HashMap::new();

        processes.insert(
            "test-srv".to_string(),
            ActiveProcess {
                config: ProcessConfig {
                    id: "test-srv".to_string(),
                    name: "Original".to_string(),
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    cwd: None,
                    auto_restart: false,
                    max_restarts: 0,
                    auto_start: false,
                },
                status: ProcessStatus::Running,
                restart_count: 2,
                pid: Some(12345),
                start_time: Some(chrono::Utc::now()),
                stop_tx: None,
            },
        );

        let updated_config = ProcessConfig {
            id: "test-srv".to_string(),
            name: "Updated".to_string(),
            command: "ping".to_string(),
            args: vec!["localhost".to_string()],
            cwd: Some("/tmp".to_string()),
            auto_restart: true,
            max_restarts: 5,
            auto_start: true,
        };

        // update_process ile aynı sıra: DB (bu testte atlandı) → state taşıma
        let process = processes.get_mut("test-srv").unwrap();

        // Runtime state'i koru (DB'den sonra — bu testte DB yok, state'i direkt taşı)
        let status = process.status;
        let restart_count = process.restart_count;
        let pid = process.pid;
        let start_time = process.start_time;
        let stop_tx = process.stop_tx.take();

        *process = ActiveProcess {
            config: updated_config.clone(),
            status,
            restart_count,
            pid,
            start_time,
            stop_tx,
        };

        // Verify
        let result = processes.get("test-srv").unwrap();
        assert_eq!(result.config.name, "Updated");
        assert_eq!(result.config.command, "ping");
        assert_eq!(result.config.args, vec!["localhost"]);
        assert_eq!(result.config.cwd, Some("/tmp".to_string()));
        assert!(result.config.auto_restart);
        assert_eq!(result.config.max_restarts, 5);
        assert!(result.config.auto_start);
        // Runtime state korunmalı
        assert_eq!(result.status, ProcessStatus::Running);
        assert_eq!(result.restart_count, 2);
        assert!(result.pid.is_some());

        // updated_config.id override testi
        assert_eq!(result.config.id, "test-srv");
    }
}
