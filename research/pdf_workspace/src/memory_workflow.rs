//! Opt-in repeatable viewing workload; emits phase markers for the external RSS sampler.
use serde::Deserialize;
use std::time::{Duration, Instant};
#[derive(Clone, Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(default)]
    pub page: i32,
    #[serde(default = "one")]
    pub zoom: f64,
    #[serde(default)]
    pub pan: [f64; 2],
    #[serde(default = "dwell")]
    pub dwell: f64,
}
fn one() -> f64 {
    1.0
}
fn dwell() -> f64 {
    2.0
}
pub struct Workflow {
    pub steps: Vec<Step>,
    pub index: usize,
    pub started: Instant,
    pub quiet: Option<Instant>,
    pub applied: bool,
    pub reported: bool,
}
impl Workflow {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var("IQS_MEMORY_WORKFLOW").ok()?;
        let steps = serde_json::from_slice(&std::fs::read(path).expect("read workflow"))
            .expect("parse workflow");
        Some(Self {
            steps,
            index: 0,
            started: Instant::now(),
            quiet: None,
            applied: false,
            reported: false,
        })
    }
    pub fn begin(&mut self) {
        self.applied = true;
        self.started = Instant::now();
        self.quiet = None;
        self.reported = false;
        println!(
            "{}",
            serde_json::json!({"event":"phase","name":self.steps[self.index].name})
        );
    }
    pub fn advance(&mut self, ready: bool) -> bool {
        if ready {
            self.quiet.get_or_insert_with(Instant::now);
        } else {
            self.quiet = None;
        }
        if self
            .quiet
            .is_some_and(|t| t.elapsed() >= Duration::from_secs_f64(self.steps[self.index].dwell))
        {
            println!(
                "{}",
                serde_json::json!({"event":"settled","name":self.steps[self.index].name,"elapsed_ms":self.started.elapsed().as_secs_f64()*1000.0})
            );
            self.index += 1;
            self.applied = false;
            self.quiet = None;
            return self.index == self.steps.len();
        }
        false
    }
}
