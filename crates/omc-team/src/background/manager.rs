use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::concurrency::ConcurrencyManager;
use super::{
    BackgroundTask, BackgroundTaskConfig, BackgroundTaskStatus, LaunchInput, ResumeContext,
    ResumeInput, TaskProgress,
};

/// Default task timeout: 30 minutes.
pub const DEFAULT_TASK_TTL_MS: u64 = 30 * 60 * 1000;

fn default_storage_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join(".omc")
        .join("background-tasks")
}

fn generate_task_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand_suffix = format!("{:06x}", ts & 0xFFFFFF);
    format!("bg_{:x}{}", ts, rand_suffix)
}

pub struct BackgroundManager {
    tasks: RwLock<HashMap<String, BackgroundTask>>,
    notifications: RwLock<HashMap<String, Vec<BackgroundTask>>>,
    concurrency: ConcurrencyManager,
    config: BackgroundTaskConfig,
    storage_dir: PathBuf,
    shutdown: Arc<Mutex<bool>>,
}

impl BackgroundManager {
    pub fn new(config: BackgroundTaskConfig) -> Self {
        Self::with_dir(config, default_storage_dir())
    }

    pub fn with_dir(config: BackgroundTaskConfig, dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);

        let mut tasks = HashMap::new();
        Self::load_persisted_tasks(&dir, &mut tasks);

        Self {
            tasks: RwLock::new(tasks),
            notifications: RwLock::new(HashMap::new()),
            concurrency: ConcurrencyManager::new(config.clone()),
            config,
            storage_dir: dir,
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    fn load_persisted_tasks(dir: &PathBuf, tasks: &mut HashMap<String, BackgroundTask>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(task) = serde_json::from_str::<BackgroundTask>(&content)
            {
                tasks.insert(task.id.clone(), task);
            }
        }
    }

    fn task_path(&self, task_id: &str) -> PathBuf {
        self.storage_dir.join(format!("{task_id}.json"))
    }

    fn persist_task(&self, task: &BackgroundTask) {
        let path = self.task_path(&task.id);
        if let Ok(json) = serde_json::to_string_pretty(task) {
            let _ = fs::write(path, json);
        }
    }

    fn unpersist_task(&self, task_id: &str) {
        let path = self.task_path(task_id);
        let _ = fs::remove_file(path);
    }

    pub fn launch(&self, input: LaunchInput) -> Result<BackgroundTask, String> {
        let concurrency_key = input.agent.clone();
        let max_total = self.config.max_total_tasks.unwrap_or(10);

        {
            let tasks = self.tasks.read().unwrap();
            let running = tasks
                .values()
                .filter(|t| t.status == BackgroundTaskStatus::Running)
                .count();
            let queued = tasks
                .values()
                .filter(|t| t.status == BackgroundTaskStatus::Queued)
                .count();

            if running + queued >= max_total {
                return Err(format!(
                    "Maximum tasks in flight ({max_total}) reached. \
                     Currently: {running} running, {queued} queued. \
                     Wait for some tasks to complete."
                ));
            }

            if let Some(max_queue) = self.config.max_queue_size
                && queued >= max_queue
            {
                return Err(format!(
                    "Maximum queue size ({max_queue}) reached. \
                         Currently: {running} running, {queued} queued. \
                         Wait for some tasks to start or complete."
                ));
            }
        }

        let task_id = generate_task_id();
        let session_id = format!("ses_{}", generate_task_id());

        let task = BackgroundTask {
            id: task_id.clone(),
            session_id,
            parent_session_id: input.parent_session_id,
            description: input.description,
            prompt: input.prompt,
            agent: input.agent,
            status: BackgroundTaskStatus::Queued,
            queued_at: Some(Utc::now()),
            started_at: Utc::now(),
            completed_at: None,
            result: None,
            error: None,
            progress: TaskProgress::default(),
            concurrency_key: Some(concurrency_key.clone()),
            parent_model: input.model,
        };

        self.tasks
            .write()
            .unwrap()
            .insert(task_id.clone(), task.clone());
        self.persist_task(&task);

        // Acquire concurrency slot (blocking via a temporary tokio runtime)
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(self.concurrency.acquire(&concurrency_key));

        // Transition to running
        {
            let mut tasks = self.tasks.write().unwrap();
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = BackgroundTaskStatus::Running;
                task.started_at = Utc::now();
                self.persist_task(task);
            }
        }

        Ok(self.tasks.read().unwrap().get(&task_id).unwrap().clone())
    }

    pub fn resume(&self, input: ResumeInput) -> Result<BackgroundTask, String> {
        let mut tasks = self.tasks.write().unwrap();
        let task = tasks
            .values_mut()
            .find(|t| t.session_id == input.session_id)
            .ok_or_else(|| format!("Task not found for session: {}", input.session_id))?;

        task.status = BackgroundTaskStatus::Running;
        task.completed_at = None;
        task.error = None;
        task.parent_session_id = input.parent_session_id;
        task.progress.last_update = Utc::now();

        self.persist_task(task);
        Ok(task.clone())
    }

    pub fn get_resume_context(&self, session_id: &str) -> Option<ResumeContext> {
        let tasks = self.tasks.read().unwrap();
        let task = tasks.values().find(|t| t.session_id == session_id)?;

        Some(ResumeContext {
            session_id: task.session_id.clone(),
            previous_prompt: task.prompt.clone(),
            tool_call_count: task.progress.tool_calls,
            last_tool_used: task.progress.last_tool.clone(),
            last_output_summary: task
                .progress
                .last_message
                .as_deref()
                .map(|m| m.chars().take(500).collect()),
            started_at: task.started_at,
            last_activity_at: task.progress.last_update,
        })
    }

    pub fn get_task(&self, id: &str) -> Option<BackgroundTask> {
        self.tasks.read().unwrap().get(id).cloned()
    }

    pub fn find_by_session(&self, session_id: &str) -> Option<BackgroundTask> {
        self.tasks
            .read()
            .unwrap()
            .values()
            .find(|t| t.session_id == session_id)
            .cloned()
    }

    pub fn get_tasks_by_parent_session(&self, session_id: &str) -> Vec<BackgroundTask> {
        self.tasks
            .read()
            .unwrap()
            .values()
            .filter(|t| t.parent_session_id == session_id)
            .cloned()
            .collect()
    }

    pub fn get_all_tasks(&self) -> Vec<BackgroundTask> {
        self.tasks.read().unwrap().values().cloned().collect()
    }

    pub fn get_running_tasks(&self) -> Vec<BackgroundTask> {
        self.tasks
            .read()
            .unwrap()
            .values()
            .filter(|t| t.status == BackgroundTaskStatus::Running)
            .cloned()
            .collect()
    }

    pub fn update_task_status(
        &self,
        task_id: &str,
        status: BackgroundTaskStatus,
        result: Option<String>,
        error: Option<String>,
    ) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status.clone();
            if let Some(r) = result {
                task.result = Some(r);
            }
            if let Some(e) = error {
                task.error = Some(e);
            }

            if matches!(
                status,
                BackgroundTaskStatus::Completed
                    | BackgroundTaskStatus::Error
                    | BackgroundTaskStatus::Cancelled
            ) {
                task.completed_at = Some(Utc::now());

                if let Some(ref key) = task.concurrency_key {
                    self.concurrency.release(key);
                }

                let task_clone = task.clone();
                drop(tasks);
                self.mark_for_notification(task_clone);
                return;
            }

            self.persist_task(task);
        }
    }

    pub fn update_task_progress(
        &self,
        task_id: &str,
        tool_calls: Option<u32>,
        last_tool: Option<String>,
        last_message: Option<String>,
    ) {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            if let Some(tc) = tool_calls {
                task.progress.tool_calls = tc;
            }
            if let Some(lt) = last_tool {
                task.progress.last_tool = Some(lt);
            }
            if let Some(lm) = last_message {
                task.progress.last_message = Some(lm);
                task.progress.last_message_at = Some(Utc::now());
            }
            task.progress.last_update = Utc::now();
            self.persist_task(task);
        }
    }

    fn mark_for_notification(&self, task: BackgroundTask) {
        let mut notifications = self.notifications.write().unwrap();
        let queue = notifications
            .entry(task.parent_session_id.clone())
            .or_default();
        queue.push(task);
    }

    pub fn get_pending_notifications(&self, session_id: &str) -> Vec<BackgroundTask> {
        self.notifications
            .read()
            .unwrap()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear_notifications(&self, session_id: &str) {
        self.notifications.write().unwrap().remove(session_id);
    }

    fn clear_notifications_for_task(&self, task_id: &str) {
        let mut notifications = self.notifications.write().unwrap();
        for tasks in notifications.values_mut() {
            tasks.retain(|t| t.id != task_id);
        }
        notifications.retain(|_, tasks| !tasks.is_empty());
    }

    pub fn remove_task(&self, task_id: &str) {
        let task = self.tasks.read().unwrap().get(task_id).cloned();
        if let Some(task) = task
            && let Some(ref key) = task.concurrency_key
        {
            self.concurrency.release(key);
        }

        self.clear_notifications_for_task(task_id);
        self.unpersist_task(task_id);
        self.tasks.write().unwrap().remove(task_id);
    }

    pub fn prune_stale_tasks(&self) {
        let now = Utc::now();
        let ttl = Duration::from_millis(self.config.task_timeout_ms.unwrap_or(DEFAULT_TASK_TTL_MS));
        let stale_threshold =
            Duration::from_millis(self.config.stale_threshold_ms.unwrap_or(5 * 60 * 1000));

        let mut tasks_to_remove = Vec::new();
        {
            let tasks = self.tasks.read().unwrap();
            for (id, task) in tasks.iter() {
                let age = now.signed_duration_since(task.started_at);
                if age.to_std().unwrap_or_default() > ttl
                    && matches!(
                        task.status,
                        BackgroundTaskStatus::Running | BackgroundTaskStatus::Queued
                    )
                {
                    tasks_to_remove.push(id.clone());
                }

                if task.status == BackgroundTaskStatus::Running {
                    let last_activity = task.progress.last_update;
                    let idle = now.signed_duration_since(last_activity);
                    if idle.to_std().unwrap_or_default() > stale_threshold * 2 {
                        tasks_to_remove.push(id.clone());
                    }
                }
            }
        }

        for task_id in &tasks_to_remove {
            let task = self.tasks.read().unwrap().get(task_id).cloned();
            if let Some(mut task) = task {
                task.status = BackgroundTaskStatus::Error;
                task.error = Some("Task timed out or became stale".to_string());
                task.completed_at = Some(Utc::now());

                if let Some(ref key) = task.concurrency_key {
                    self.concurrency.release(key);
                }

                self.clear_notifications_for_task(task_id);
                self.unpersist_task(task_id);
            }
            self.tasks.write().unwrap().remove(task_id);
        }

        // Prune old notifications
        let mut notifications = self.notifications.write().unwrap();
        for tasks in notifications.values_mut() {
            tasks.retain(|t| {
                let age = now.signed_duration_since(t.started_at);
                age.to_std().unwrap_or_default() <= ttl
            });
        }
        notifications.retain(|_, tasks| !tasks.is_empty());
    }

    pub fn format_duration(start: DateTime<Utc>, end: Option<DateTime<Utc>>) -> String {
        let end = end.unwrap_or_else(Utc::now);
        let duration = end.signed_duration_since(start);
        let total_secs = duration.num_seconds().max(0) as u64;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{minutes}m {seconds}s")
        } else {
            format!("{seconds}s")
        }
    }

    pub fn get_status_summary(&self) -> String {
        let tasks = self.tasks.read().unwrap();
        let running = tasks
            .values()
            .filter(|t| t.status == BackgroundTaskStatus::Running)
            .count();
        let queued = tasks
            .values()
            .filter(|t| t.status == BackgroundTaskStatus::Queued)
            .count();
        let total = tasks.len();

        if total == 0 {
            return "No background tasks.".to_string();
        }

        let mut lines = vec![format!(
            "Background Tasks: {running} running, {queued} queued, {total} total"
        )];
        lines.push(String::new());

        for task in tasks.values() {
            let duration = Self::format_duration(task.started_at, task.completed_at);
            let status = format!("{:?}", task.status).to_uppercase();
            let progress = format!(" ({} tools)", task.progress.tool_calls);

            lines.push(format!(
                "  [{status}] {} - {duration}{progress}",
                task.description
            ));

            if let Some(ref err) = task.error {
                lines.push(format!("    Error: {err}"));
            }
        }

        lines.join("\n")
    }

    pub fn cleanup(&self) {
        *self.shutdown.lock().unwrap() = true;
        self.tasks.write().unwrap().clear();
        self.notifications.write().unwrap().clear();
        self.concurrency.clear();
    }
}

#[allow(dead_code)]
static INSTANCE: std::sync::OnceLock<BackgroundManager> = std::sync::OnceLock::new();

#[allow(dead_code)]
pub fn get_background_manager(config: BackgroundTaskConfig) -> &'static BackgroundManager {
    INSTANCE.get_or_init(|| BackgroundManager::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BackgroundTaskConfig {
        BackgroundTaskConfig {
            default_concurrency: Some(2),
            max_total_tasks: Some(5),
            ..Default::default()
        }
    }

    fn temp_dir() -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("omc-bg-test-{ts}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn test_manager(config: BackgroundTaskConfig) -> (BackgroundManager, PathBuf) {
        let dir = temp_dir();
        let mgr = BackgroundManager::with_dir(config, dir.clone());
        (mgr, dir)
    }

    fn cleanup_dir(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn launch_creates_queued_then_running_task() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "test task".to_string(),
                prompt: "do something".to_string(),
                agent: "executor".to_string(),
                parent_session_id: "parent-1".to_string(),
                model: None,
            })
            .unwrap();

        assert_eq!(task.status, BackgroundTaskStatus::Running);
        assert_eq!(task.description, "test task");
        assert!(task.concurrency_key.is_some());
        cleanup_dir(&dir);
    }

    #[test]
    fn get_task_returns_launched_task() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "findable".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        let found = mgr.get_task(&task.id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().description, "findable");
        cleanup_dir(&dir);
    }

    #[test]
    fn find_by_session_works() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "session test".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        let found = mgr.find_by_session(&task.session_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, task.session_id);
        cleanup_dir(&dir);
    }

    #[test]
    fn get_tasks_by_parent_session() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(10),
            max_total_tasks: Some(20),
            ..Default::default()
        };
        let (mgr, dir) = test_manager(config);
        for i in 0..3 {
            mgr.launch(LaunchInput {
                description: format!("task-{i}"),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "parent-shared".to_string(),
                model: None,
            })
            .unwrap();
        }

        let tasks = mgr.get_tasks_by_parent_session("parent-shared");
        assert_eq!(tasks.len(), 3);
        cleanup_dir(&dir);
    }

    #[test]
    fn update_task_status_marks_completion() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "complete me".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        mgr.update_task_status(
            &task.id,
            BackgroundTaskStatus::Completed,
            Some("done".to_string()),
            None,
        );

        let updated = mgr.get_task(&task.id).unwrap();
        assert_eq!(updated.status, BackgroundTaskStatus::Completed);
        assert_eq!(updated.result.as_deref(), Some("done"));
        assert!(updated.completed_at.is_some());
        cleanup_dir(&dir);
    }

    #[test]
    fn update_task_progress_tracks_tool_calls() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "progress test".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        mgr.update_task_progress(
            &task.id,
            Some(5),
            Some("bash".to_string()),
            Some("ran tests".to_string()),
        );

        let updated = mgr.get_task(&task.id).unwrap();
        assert_eq!(updated.progress.tool_calls, 5);
        assert_eq!(updated.progress.last_tool.as_deref(), Some("bash"));
        cleanup_dir(&dir);
    }

    #[test]
    fn remove_task_cleans_up() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "removable".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        mgr.remove_task(&task.id);
        assert!(mgr.get_task(&task.id).is_none());
        cleanup_dir(&dir);
    }

    #[test]
    fn max_total_tasks_enforced() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(100),
            max_total_tasks: Some(2),
            ..Default::default()
        };
        let (mgr, dir) = test_manager(config);

        mgr.launch(LaunchInput {
            description: "t1".to_string(),
            prompt: "p".to_string(),
            agent: "a".to_string(),
            parent_session_id: "p".to_string(),
            model: None,
        })
        .unwrap();

        mgr.launch(LaunchInput {
            description: "t2".to_string(),
            prompt: "p".to_string(),
            agent: "a".to_string(),
            parent_session_id: "p".to_string(),
            model: None,
        })
        .unwrap();

        let err = mgr
            .launch(LaunchInput {
                description: "t3".to_string(),
                prompt: "p".to_string(),
                agent: "a".to_string(),
                parent_session_id: "p".to_string(),
                model: None,
            })
            .unwrap_err();

        assert!(err.contains("Maximum tasks in flight"));
        cleanup_dir(&dir);
    }

    #[test]
    fn notifications_for_completed_tasks() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "notify test".to_string(),
                prompt: "test".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "parent-notify".to_string(),
                model: None,
            })
            .unwrap();

        mgr.update_task_status(
            &task.id,
            BackgroundTaskStatus::Completed,
            Some("done".to_string()),
            None,
        );

        let notifications = mgr.get_pending_notifications("parent-notify");
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].id, task.id);

        mgr.clear_notifications("parent-notify");
        assert!(mgr.get_pending_notifications("parent-notify").is_empty());
        cleanup_dir(&dir);
    }

    #[test]
    fn get_status_summary_empty() {
        let (mgr, dir) = test_manager(test_config());
        assert_eq!(mgr.get_status_summary(), "No background tasks.");
        cleanup_dir(&dir);
    }

    #[test]
    fn get_status_summary_with_tasks() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(100),
            ..Default::default()
        };
        let (mgr, dir) = test_manager(config);

        mgr.launch(LaunchInput {
            description: "running task".to_string(),
            prompt: "test".to_string(),
            agent: "agent".to_string(),
            parent_session_id: "p".to_string(),
            model: None,
        })
        .unwrap();

        let summary = mgr.get_status_summary();
        assert!(summary.contains("1 running"));
        assert!(summary.contains("running task"));
        cleanup_dir(&dir);
    }

    #[test]
    fn resume_restores_running_status() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "resumable".to_string(),
                prompt: "original".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        mgr.update_task_status(
            &task.id,
            BackgroundTaskStatus::Completed,
            Some("partial".to_string()),
            None,
        );

        let resumed = mgr
            .resume(ResumeInput {
                session_id: task.session_id.clone(),
                prompt: "continue".to_string(),
                parent_session_id: "p1".to_string(),
            })
            .unwrap();

        assert_eq!(resumed.status, BackgroundTaskStatus::Running);
        assert!(resumed.completed_at.is_none());
        cleanup_dir(&dir);
    }

    #[test]
    fn get_resume_context() {
        let (mgr, dir) = test_manager(test_config());
        let task = mgr
            .launch(LaunchInput {
                description: "context test".to_string(),
                prompt: "do work".to_string(),
                agent: "agent".to_string(),
                parent_session_id: "p1".to_string(),
                model: None,
            })
            .unwrap();

        let ctx = mgr.get_resume_context(&task.session_id).unwrap();
        assert_eq!(ctx.previous_prompt, "do work");
        assert_eq!(ctx.session_id, task.session_id);
        cleanup_dir(&dir);
    }

    #[test]
    fn format_duration_variants() {
        let start = Utc::now() - chrono::Duration::seconds(3661);
        assert!(BackgroundManager::format_duration(start, None).contains("1h 1m 1s"));

        let start = Utc::now() - chrono::Duration::seconds(65);
        assert!(BackgroundManager::format_duration(start, None).contains("1m 5s"));

        let start = Utc::now() - chrono::Duration::seconds(5);
        assert!(BackgroundManager::format_duration(start, None).contains("5s"));
    }

    #[test]
    fn cleanup_clears_all_state() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(100),
            ..Default::default()
        };
        let (mgr, dir) = test_manager(config);

        mgr.launch(LaunchInput {
            description: "will be cleaned".to_string(),
            prompt: "test".to_string(),
            agent: "agent".to_string(),
            parent_session_id: "p1".to_string(),
            model: None,
        })
        .unwrap();

        mgr.cleanup();
        assert!(mgr.get_all_tasks().is_empty());
        cleanup_dir(&dir);
    }
}
