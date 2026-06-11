use serde::{Deserialize, Serialize};
use super::modflow_simple::{FlowFieldResult, GridCell, WellPoint};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContaminationPath {
    pub source_id: String,
    pub waypoints: Vec<PathWaypoint>,
    pub total_distance_m: f64,
    pub total_time_days: f64,
    pub max_concentration_ppm: f64,
    pub arrival_time_to_sensitive_zones: Vec<ArrivalAlert>,
    pub risk_level: ContaminationRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathWaypoint {
    pub x: f64,
    pub y: f64,
    pub time_days: f64,
    pub concentration_ppm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrivalAlert {
    pub zone_name: String,
    pub arrival_days: f64,
    pub peak_ppm: f64,
    pub risk_level: ContaminationRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContaminationRisk {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl ContaminationRisk {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContaminationRisk::None => "无",
            ContaminationRisk::Low => "低",
            ContaminationRisk::Medium => "中",
            ContaminationRisk::High => "高",
            ContaminationRisk::Critical => "严重",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffusionResult {
    pub time_series: Vec<DiffusionTimeStep>,
    pub final_concentration_grid: Vec<ConcentrationCell>,
    pub contamination_paths: Vec<ContaminationPath>,
    pub sensitive_zones: Vec<SensitiveZone>,
    pub overall_warning: GroundwaterWarning,
    pub threshold_ppm: f64,
    pub total_simulation_days: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationCell {
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub concentration_ppm: f64,
    pub exceed_threshold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffusionTimeStep {
    pub time_days: f64,
    pub total_mass_kg: f64,
    pub max_concentration_ppm: f64,
    pub affected_cells: usize,
    pub plume_centroid_x: f64,
    pub plume_centroid_y: f64,
    pub plume_radius_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveZone {
    pub id: String,
    pub name: String,
    pub x_center: f64,
    pub y_center: f64,
    pub radius_m: f64,
    pub zone_type: String,
    pub artifact_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundwaterWarning {
    pub has_warning: bool,
    pub warning_level: ContaminationRisk,
    pub affected_sensitive_zones: Vec<String>,
    pub time_to_first_impact_days: f64,
    pub mitigation_suggestions: Vec<String>,
}

pub struct ChlorideTransport {
    dt_days: f64,
    diffusion_coeff_m2_s: f64,
    dispersivity_m: f64,
    retardation_factor: f64,
    decay_rate_per_day: f64,
}

impl ChlorideTransport {
    pub fn new() -> Self {
        Self {
            dt_days: 0.5,
            diffusion_coeff_m2_s: 1.5e-9,
            dispersivity_m: 2.0,
            retardation_factor: 1.2,
            decay_rate_per_day: 0.0005,
        }
    }

    pub fn with_params(
        diffusion_coeff_m2_s: f64,
        dispersivity_m: f64,
        retardation_factor: f64,
    ) -> Self {
        Self {
            dt_days: 0.5,
            diffusion_coeff_m2_s,
            dispersivity_m,
            retardation_factor,
            decay_rate_per_day: 0.0005,
        }
    }

    pub fn simulate(
        &self,
        flow_field: &FlowFieldResult,
        total_days: f64,
        threshold_ppm: f64,
        sensitive_zones: &[SensitiveZone],
    ) -> DiffusionResult {
        let n = flow_field.grid.len();
        let mut concentration = vec![0.0_f64; n];
        let rows = flow_field.grid_rows;
        let cols = flow_field.grid_cols;
        let cell_size = flow_field.cell_size_m;

        for w in &flow_field.wells {
            if matches!(w.well_type, super::modflow_simple::WellType::ContaminationSource) {
                let idx = w.row * cols + w.col;
                if idx < n {
                    concentration[idx] = w.concentration_ppm;
                }
            }
        }

        let num_steps = (total_days / self.dt_days) as usize;
        let sample_interval = (num_steps / 40).max(1);

        let mut time_series = Vec::with_capacity(40);

        for step in 0..num_steps {
            self.advection_diffusion_step(
                &mut concentration,
                &flow_field.grid,
                rows,
                cols,
                cell_size,
            );

            for w in &flow_field.wells {
                if matches!(w.well_type, super::modflow_simple::WellType::ContaminationSource) {
                    let idx = w.row * cols + w.col;
                    if idx < n {
                        let continuous_source = w.concentration_ppm * 0.05;
                        concentration[idx] = concentration[idx].max(continuous_source);
                    }
                }
            }

            for c in concentration.iter_mut() {
                *c *= 1.0 - self.decay_rate_per_day * self.dt_days;
                *c = c.max(0.0);
            }

            if step % sample_interval == 0 || step == num_steps - 1 {
                let t = step as f64 * self.dt_days;
                time_series.push(self.compute_time_stats(
                    t,
                    &concentration,
                    &flow_field.grid,
                    cell_size,
                    rows,
                    cols,
                ));
            }
        }

        let final_grid: Vec<ConcentrationCell> = flow_field
            .grid
            .iter()
            .enumerate()
            .map(|(i, g)| ConcentrationCell {
                row: g.row,
                col: g.col,
                x: g.x,
                y: g.y,
                concentration_ppm: concentration[i],
                exceed_threshold: concentration[i] > threshold_ppm,
            })
            .collect();

        let contamination_paths = self.trace_contamination_paths(
            flow_field,
            &concentration,
            total_days,
            threshold_ppm,
            rows,
            cols,
            cell_size,
        );

        let warning = self.compute_warning(
            &final_grid,
            sensitive_zones,
            &time_series,
            threshold_ppm,
        );

        DiffusionResult {
            time_series,
            final_concentration_grid: final_grid,
            contamination_paths,
            sensitive_zones: sensitive_zones.to_vec(),
            overall_warning: warning,
            threshold_ppm,
            total_simulation_days: total_days,
        }
    }

    fn advection_diffusion_step(
        &self,
        c: &mut Vec<f64>,
        grid: &[GridCell],
        rows: usize,
        cols: usize,
        dx: f64,
    ) {
        let dt_seconds = self.dt_days * 86400.0;
        let n = c.len();
        let mut new_c = c.clone();

        let molecular_diffusion = self.diffusion_coeff_m2_s;
        let porosity = 0.25;
        let retardation = self.retardation_factor;

        for r in 1..rows - 1 {
            for col in 1..cols - 1 {
                let idx = r * cols + col;
                let cell = &grid[idx];

                let theta_deg = cell.flow_direction_deg;
                let vel_m_s = cell.flow_velocity_m_d / 86400.0 / retardation;
                let theta_rad = theta_deg.to_radians();
                let vx = vel_m_s * theta_rad.cos();
                let vy = vel_m_s * theta_rad.sin();

                let alpha_l = self.dispersivity_m;
                let alpha_t = self.dispersivity_m * 0.1;
                let dxx = molecular_diffusion + alpha_l * vx.abs() + alpha_t * vy.abs();
                let dyy = molecular_diffusion + alpha_l * vy.abs() + alpha_t * vx.abs();

                let c_up = if r > 0 { c[idx - cols] } else { c[idx] };
                let c_down = if r < rows - 1 { c[idx + cols] } else { c[idx] };
                let c_left = if col > 0 { c[idx - 1] } else { c[idx] };
                let c_right = if col < cols - 1 { c[idx + 1] } else { c[idx] };

                let d2c_dx2 = (c_right - 2.0 * c[idx] + c_left) / (dx * dx);
                let d2c_dy2 = (c_down - 2.0 * c[idx] + c_up) / (dx * dx);
                let dc_dx = (c_right - c_left) / (2.0 * dx);
                let dc_dy = (c_down - c_up) / (2.0 * dx);

                let diffusion = dxx * d2c_dx2 + dyy * d2c_dy2;
                let advection = -(vx * dc_dx + vy * dc_dy);

                let dc_dt = (diffusion + advection) / porosity;

                new_c[idx] = c[idx] + dc_dt * dt_seconds;
                new_c[idx] = new_c[idx].max(0.0);
            }
        }

        for r in 0..rows {
            for col in 0..cols {
                let idx = r * cols + col;
                if idx < n {
                    c[idx] = new_c[idx];
                }
            }
        }
    }

    fn compute_time_stats(
        &self,
        t_days: f64,
        concentration: &[f64],
        grid: &[GridCell],
        cell_size: f64,
        rows: usize,
        cols: usize,
    ) -> DiffusionTimeStep {
        let mut max_c = 0.0_f64;
        let mut total_mass = 0.0_f64;
        let mut affected = 0usize;
        let mut centroid_x = 0.0_f64;
        let mut centroid_y = 0.0_f64;
        let mut weighted_sum = 0.0_f64;

        for (i, c) in concentration.iter().enumerate() {
            max_c = max_c.max(*c);
            if *c > 1.0 {
                affected += 1;
                let cell_mass = c * 1e-3 * 1e-3 * cell_size * cell_size * 5.0 * 0.25;
                total_mass += cell_mass;
                centroid_x += grid[i].x * c;
                centroid_y += grid[i].y * c;
                weighted_sum += c;
            }
        }

        let (cx, cy, radius) = if weighted_sum > 0.0 {
            let cx = centroid_x / weighted_sum;
            let cy = centroid_y / weighted_sum;
            let mut sum_r2 = 0.0_f64;
            let mut count = 0usize;
            for (i, c) in concentration.iter().enumerate() {
                if *c > 1.0 {
                    let dx = grid[i].x - cx;
                    let dy = grid[i].y - cy;
                    sum_r2 += dx * dx + dy * dy;
                    count += 1;
                }
            }
            let radius = if count > 0 {
                (sum_r2 / count as f64).sqrt()
            } else {
                0.0
            };
            (cx, cy, radius)
        } else {
            (0.0, 0.0, 0.0)
        };

        let _ = (rows, cols);

        DiffusionTimeStep {
            time_days: t_days,
            total_mass_kg: total_mass,
            max_concentration_ppm: max_c,
            affected_cells: affected,
            plume_centroid_x: cx,
            plume_centroid_y: cy,
            plume_radius_m: radius,
        }
    }

    fn trace_contamination_paths(
        &self,
        flow_field: &FlowFieldResult,
        final_concentration: &[f64],
        total_days: f64,
        threshold_ppm: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
    ) -> Vec<ContaminationPath> {
        let mut paths = Vec::new();
        let sources: Vec<&WellPoint> = flow_field
            .wells
            .iter()
            .filter(|w| matches!(w.well_type, super::modflow_simple::WellType::ContaminationSource))
            .collect();

        for src in sources {
            let mut waypoints = Vec::new();
            let mut r = src.row;
            let mut col = src.col;
            let mut total_dist = 0.0_f64;
            let mut total_time = 0.0_f64;
            let mut max_c = src.concentration_ppm;

            waypoints.push(PathWaypoint {
                x: flow_field.grid[r * cols + col].x,
                y: flow_field.grid[r * cols + col].y,
                time_days: 0.0,
                concentration_ppm: src.concentration_ppm,
            });

            for _ in 0..100 {
                let idx = r * cols + col;
                if idx >= flow_field.grid.len() {
                    break;
                }
                let cell = &flow_field.grid[idx];
                let dir_rad = cell.flow_direction_deg.to_radians();
                let vx = dir_rad.cos();
                let vy = dir_rad.sin();

                let next_r = if vy.abs() > vx.abs() {
                    if vy > 0.0 { r + 1 } else { r.saturating_sub(1) }
                } else {
                    r
                };
                let next_c = if vx.abs() >= vy.abs() {
                    if vx > 0.0 { col + 1 } else { col.saturating_sub(1) }
                } else {
                    col
                };

                if next_r >= rows || next_c >= cols || (next_r == r && next_c == col) {
                    break;
                }

                let step_dist = cell_size;
                let step_time = if cell.flow_velocity_m_d > 1e-6 {
                    step_dist / cell.flow_velocity_m_d
                } else {
                    1000.0
                };

                total_dist += step_dist;
                total_time += step_time;

                if total_time > total_days {
                    break;
                }

                let next_idx = next_r * cols + next_c;
                let c = if next_idx < final_concentration.len() {
                    final_concentration[next_idx]
                } else {
                    0.0
                };
                max_c = max_c.max(c);

                waypoints.push(PathWaypoint {
                    x: flow_field.grid[next_idx.min(flow_field.grid.len() - 1)].x,
                    y: flow_field.grid[next_idx.min(flow_field.grid.len() - 1)].y,
                    time_days: total_time,
                    concentration_ppm: c,
                });

                r = next_r;
                col = next_c;

                if c < threshold_ppm * 0.1 && waypoints.len() > 10 {
                    break;
                }
            }

            let arrival_alerts = self.compute_zone_arrivals(
                &waypoints,
                threshold_ppm,
                &self.default_sensitive_zones(&flow_field.origin_x, &flow_field.origin_y),
            );

            let worst_risk = arrival_alerts
                .iter()
                .map(|a| a.risk_level)
                .max()
                .unwrap_or(ContaminationRisk::None);

            paths.push(ContaminationPath {
                source_id: src.id.clone(),
                waypoints,
                total_distance_m: total_dist,
                total_time_days: total_time,
                max_concentration_ppm: max_c,
                arrival_time_to_sensitive_zones: arrival_alerts,
                risk_level: worst_risk,
            });
        }

        paths
    }

    fn compute_zone_arrivals(
        &self,
        waypoints: &[PathWaypoint],
        threshold_ppm: f64,
        zones: &[SensitiveZone],
    ) -> Vec<ArrivalAlert> {
        let mut alerts = Vec::new();

        for zone in zones {
            for wp in waypoints {
                let dx = wp.x - zone.x_center;
                let dy = wp.y - zone.y_center;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < zone.radius_m {
                    let risk = if wp.concentration_ppm > threshold_ppm * 3.0 {
                        ContaminationRisk::Critical
                    } else if wp.concentration_ppm > threshold_ppm * 2.0 {
                        ContaminationRisk::High
                    } else if wp.concentration_ppm > threshold_ppm {
                        ContaminationRisk::Medium
                    } else if wp.concentration_ppm > threshold_ppm * 0.5 {
                        ContaminationRisk::Low
                    } else {
                        ContaminationRisk::None
                    };

                    alerts.push(ArrivalAlert {
                        zone_name: zone.name.clone(),
                        arrival_days: wp.time_days,
                        peak_ppm: wp.concentration_ppm,
                        risk_level: risk,
                    });
                    break;
                }
            }
        }

        alerts
    }

    fn default_sensitive_zones(&self, origin_x: &f64, origin_y: &f64) -> Vec<SensitiveZone> {
        vec![
            SensitiveZone {
                id: "SZ-01".to_string(),
                name: "主营区文物密集带".to_string(),
                x_center: origin_x + 50.0,
                y_center: origin_y + 40.0,
                radius_m: 12.0,
                zone_type: "文物密集".to_string(),
                artifact_count: 120,
            },
            SensitiveZone {
                id: "SZ-02".to_string(),
                name: "器械库出土点".to_string(),
                x_center: origin_x + 90.0,
                y_center: origin_y + 55.0,
                radius_m: 10.0,
                zone_type: "出土点".to_string(),
                artifact_count: 85,
            },
            SensitiveZone {
                id: "SZ-03".to_string(),
                name: "手术区医疗器械坑".to_string(),
                x_center: origin_x + 70.0,
                y_center: origin_y + 75.0,
                radius_m: 8.0,
                zone_type: "考古坑".to_string(),
                artifact_count: 60,
            },
            SensitiveZone {
                id: "SZ-04".to_string(),
                name: "药房药物残留区".to_string(),
                x_center: origin_x + 30.0,
                y_center: origin_y + 65.0,
                radius_m: 9.0,
                zone_type: "特殊遗存".to_string(),
                artifact_count: 40,
            },
        ]
    }

    fn compute_warning(
        &self,
        final_grid: &[ConcentrationCell],
        sensitive_zones: &[SensitiveZone],
        time_series: &[DiffusionTimeStep],
        threshold_ppm: f64,
    ) -> GroundwaterWarning {
        let mut affected_zones = Vec::new();
        let mut first_impact = f64::INFINITY;
        let mut max_risk = ContaminationRisk::None;

        for zone in sensitive_zones {
            let mut zone_exceeded = false;
            for cell in final_grid {
                let dx = cell.x - zone.x_center;
                let dy = cell.y - zone.y_center;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < zone.radius_m && cell.concentration_ppm > threshold_ppm {
                    zone_exceeded = true;
                    break;
                }
            }
            if zone_exceeded {
                affected_zones.push(zone.name.clone());
            }
        }

        for ts in time_series {
            if ts.max_concentration_ppm > threshold_ppm && ts.time_days < first_impact {
                first_impact = ts.time_days;
                break;
            }
        }

        let worst_max = time_series
            .iter()
            .map(|t| t.max_concentration_ppm)
            .fold(0.0_f64, f64::max);

        let overall_risk = if worst_max > threshold_ppm * 5.0 {
            ContaminationRisk::Critical
        } else if worst_max > threshold_ppm * 3.0 {
            ContaminationRisk::High
        } else if worst_max > threshold_ppm {
            ContaminationRisk::Medium
        } else if worst_max > threshold_ppm * 0.5 {
            ContaminationRisk::Low
        } else {
            ContaminationRisk::None
        };

        max_risk = overall_risk;

        let suggestions = self.generate_suggestions(overall_risk, &affected_zones);

        GroundwaterWarning {
            has_warning: !matches!(overall_risk, ContaminationRisk::None),
            warning_level: max_risk,
            affected_sensitive_zones: affected_zones,
            time_to_first_impact_days: if first_impact.is_infinite() { -1.0 } else { first_impact },
            mitigation_suggestions: suggestions,
        }
    }

    fn generate_suggestions(
        &self,
        risk: ContaminationRisk,
        affected: &[String],
    ) -> Vec<String> {
        let mut recs = Vec::new();

        match risk {
            ContaminationRisk::None => {
                recs.push("当前氯离子浓度在安全范围内，按常规季度监测即可".to_string());
            }
            ContaminationRisk::Low => {
                recs.push("检测到低浓度氯离子扩散，建议加密地下水监测频率".to_string());
            }
            ContaminationRisk::Medium => {
                recs.push("氯离子浓度接近临界值，建议启动预防性抽水治理".to_string());
                recs.push("在污染源周边设置反应渗透墙(PRB)阻断扩散路径".to_string());
            }
            ContaminationRisk::High => {
                recs.push("氯离子扩散风险较高，应立即启动应急响应机制".to_string());
                recs.push("对受威胁文物实施临时覆盖隔离措施".to_string());
                recs.push("考虑注入纳米零价铁或生物炭进行原位修复".to_string());
            }
            ContaminationRisk::Critical => {
                recs.push("【紧急】氯离子严重超标，立即启动文物抢救程序".to_string());
                recs.push("对受影响区域文物进行紧急提取和实验室保护".to_string());
                recs.push("实施大规模抽水-处理-回灌系统控制污染羽扩散".to_string());
            }
        }

        if !affected.is_empty() {
            recs.push(format!(
                "受影响保护区: {}",
                affected.join("、")
            ));
        }

        recs
    }
}

impl Default for ChlorideTransport {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_sensitive_zones_list() -> Vec<SensitiveZone> {
    let ct = ChlorideTransport::new();
    ct.default_sensitive_zones(&0.0, &0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffusion_simulation() {
        let (rows, cols, size, wells) = super::modflow_simple::default_simulation_params();
        let model = super::modflow_simple::GroundwaterModel::new(rows, cols, size);
        let flow = model.solve_steady_state(15.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0);
        let zones = default_sensitive_zones_list();
        let transport = ChlorideTransport::new();
        let result = transport.simulate(&flow, 90.0, 100.0, &zones);
        assert!(result.time_series.len() > 0);
    }
}
