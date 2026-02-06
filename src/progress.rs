use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "phase")]
    Phase { name: String },
    #[serde(rename = "phase_done")]
    PhaseDone { name: String },
    #[serde(rename = "cache_start")]
    CacheStart { total: usize },
    #[serde(rename = "task_start")]
    TaskStart { name: String },
    #[serde(rename = "task_done")]
    TaskDone { name: String },
    #[serde(rename = "done")]
    Done,
}
