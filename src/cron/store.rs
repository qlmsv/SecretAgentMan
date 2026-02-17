use anyhow::Result;
use crate::config::Config;
use crate::cron::types::{CronJob, CronRun, CronJobPatch, Schedule, SessionTarget, DeliveryConfig};
use chrono::{DateTime, Utc};

pub fn list_jobs(_config: &Config) -> Result<Vec<CronJob>> {
    Ok(Vec::new())
}

pub fn add_job(_config: &Config, _expression: &str, _command: &str) -> Result<CronJob> {
    anyhow::bail!("Cron not supported in SaaS yet")
}

pub fn add_shell_job(_config: &Config, _name: Option<String>, _schedule: Schedule, _command: &str) -> Result<CronJob> {
    anyhow::bail!("Cron not supported")
}

pub fn add_agent_job(
    _config: &Config,
    _name: Option<String>,
    _schedule: Schedule,
    _prompt: &str,
    _session: SessionTarget,
    _model: Option<String>,
    _delivery: Option<DeliveryConfig>,
    _delete_after_run: bool
) -> Result<CronJob> {
    anyhow::bail!("Cron not supported")
}

pub fn remove_job(_config: &Config, _id: &str) -> Result<()> {
    Ok(())
}

pub fn update_job(_config: &Config, _id: &str, _patch: CronJobPatch) -> Result<CronJob> {
    anyhow::bail!("Not supported")
}

pub fn get_job(_config: &Config, _id: &str) -> Result<CronJob> {
    anyhow::bail!("Not found")
}

pub fn record_run(_config: &Config, _job_id: &str, _start: DateTime<Utc>, _end: DateTime<Utc>, _status: &str, _output: Option<&str>, _duration_ms: i64) -> Result<()> {
    Ok(())
}

// Changed output to &str to match callers
pub fn record_last_run(_config: &Config, _job_id: &str, _finished_at: DateTime<Utc>, _success: bool, _output: &str) -> Result<()> {
    Ok(())
}

pub fn list_runs(_config: &Config, _job_id: &str, _limit: usize) -> Result<Vec<CronRun>> {
    Ok(Vec::new())
}

pub fn prune_runs(_config: &Config, _job_id: &str, _keep: usize) -> Result<()> {
    Ok(())
}

pub fn due_jobs(_config: &Config, _now: DateTime<Utc>) -> Result<Vec<CronJob>> {
    Ok(Vec::new())
}

pub fn reschedule_after_run(_config: &Config, _job: &CronJob, _success: bool, _output: &str) -> Result<()> {
    Ok(())
}

pub fn next_run(_expression: &str) -> Result<DateTime<Utc>> {
    Ok(Utc::now())
}
