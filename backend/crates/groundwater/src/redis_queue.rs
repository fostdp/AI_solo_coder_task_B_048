use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use super::modflow_simple::{GroundwaterModel, default_simulation_params};
use super::chloride_transport::{ChlorideTransport, SensitiveZone, default_sensitive_zones_list};

pub struct GroundwaterTaskQueue {
    task_counter: AtomicU64,
    pending_tasks: Arc<Mutex<Vec<String>>>,
    completed_tasks: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl GroundwaterTaskQueue {
    pub fn new() -> Self {
        Self {
            task_counter: AtomicU64::new(0),
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            completed_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn submit_task(&self, task_type: &str, params: serde_json::Value) -> String {
        let id = self.task_counter.fetch_add(1, Ordering::SeqCst);
        let task_id = format!("gw-task-{}", id);

        {
            let mut pending = self.pending_tasks.lock().unwrap();
            pending.push(task_id.clone());
        }

        let pending_clone = Arc::clone(&self.pending_tasks);
        let completed_clone = Arc::clone(&self.completed_tasks);
        let task_type_owned = task_type.to_string();
        let tid = task_id.clone();

        std::thread::spawn(move || {
            let result = match task_type_owned.as_str() {
                "groundwater" | "modflow" => {
                    let (rows, cols, cell_size, wells) = default_simulation_params();
                    let model = GroundwaterModel::new(rows, cols, cell_size);
                    let top_head = params["top_boundary_head"].as_f64().unwrap_or(15.0);
                    let bottom_head = params["bottom_boundary_head"].as_f64().unwrap_or(10.0);
                    let conductivity = params["conductivity"].as_f64().unwrap_or(1e-5);
                    let flow = model.solve_steady_state(
                        top_head, bottom_head, None, None, conductivity, None, &wells, 0.0, 0.0,
                    );
                    serde_json::to_value(&flow).unwrap()
                }
                "chloride" | "chloride_transport" => {
                    let (rows, cols, cell_size, wells) = default_simulation_params();
                    let model = GroundwaterModel::new(rows, cols, cell_size);
                    let top_head = params["top_boundary_head"].as_f64().unwrap_or(15.0);
                    let bottom_head = params["bottom_boundary_head"].as_f64().unwrap_or(10.0);
                    let conductivity = params["conductivity"].as_f64().unwrap_or(1e-5);
                    let total_days = params["total_days"].as_f64().unwrap_or(90.0);
                    let threshold_ppm = params["threshold_ppm"].as_f64().unwrap_or(100.0);
                    let flow = model.solve_steady_state(
                        top_head, bottom_head, None, None, conductivity, None, &wells, 0.0, 0.0,
                    );
                    let zones = default_sensitive_zones_list();
                    let transport = ChlorideTransport::new();
                    let diffusion = transport.simulate(&flow, total_days, threshold_ppm, &zones);
                    serde_json::to_value(&diffusion).unwrap()
                }
                _ => {
                    serde_json::json!({"error": format!("unknown task type: {}", task_type_owned)})
                }
            };

            {
                let mut completed = completed_clone.lock().unwrap();
                completed.insert(tid.clone(), result);
            }
            {
                let mut pending = pending_clone.lock().unwrap();
                pending.retain(|t| t != &tid);
            }
        });

        task_id
    }

    pub fn get_result(&self, task_id: &str) -> Option<serde_json::Value> {
        let completed = self.completed_tasks.lock().unwrap();
        completed.get(task_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_and_get_result() {
        let queue = GroundwaterTaskQueue::new();
        let task_id = queue.submit_task("groundwater", serde_json::json!({}));
        std::thread::sleep(std::time::Duration::from_millis(500));
        let result = queue.get_result(&task_id);
        assert!(result.is_some());
    }

    #[test]
    fn test_unknown_task_returns_none() {
        let queue = GroundwaterTaskQueue::new();
        let result = queue.get_result("non-existent-id");
        assert!(result.is_none());
    }
}
