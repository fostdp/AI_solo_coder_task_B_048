use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCell {
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub hydraulic_head_m: f64,
    pub hydraulic_conductivity_m_s: f64,
    pub porosity: f64,
    pub thickness_m: f64,
    pub flow_direction_deg: f64,
    pub flow_velocity_m_d: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellPoint {
    pub id: String,
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub well_type: WellType,
    pub discharge_rate_m3_d: f64,
    pub concentration_ppm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WellType {
    #[serde(rename = "补给井")]
    Recharge,
    #[serde(rename = "抽水井")]
    Pumping,
    #[serde(rename = "污染源")]
    ContaminationSource,
    #[serde(rename = "监测井")]
    Monitor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowFieldResult {
    pub grid_rows: usize,
    pub grid_cols: usize,
    pub cell_size_m: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub grid: Vec<GridCell>,
    pub wells: Vec<WellPoint>,
    pub avg_velocity_m_d: f64,
    pub max_velocity_m_d: f64,
    pub avg_head_m: f64,
    pub head_gradient: f64,
    pub darcy_flow_summary: Vec<FlowArrow>,
    pub travel_time_days: Vec<TravelTimePoint>,
    pub convergence_status: bool,
    pub iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowArrow {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub magnitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelTimePoint {
    pub source_id: String,
    pub target_id: String,
    pub distance_m: f64,
    pub travel_days: f64,
    pub path_quality: PathQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathQuality {
    Direct,
    Diversion,
    Convergence,
    Trapped,
}

pub struct GroundwaterModel {
    rows: usize,
    cols: usize,
    cell_size: f64,
    tol: f64,
    max_iter: usize,
}

impl GroundwaterModel {
    pub fn new(rows: usize, cols: usize, cell_size_m: f64) -> Self {
        Self {
            rows,
            cols,
            cell_size: cell_size_m,
            tol: 1e-4,
            max_iter: 2000,
        }
    }

    pub fn solve_steady_state(
        &self,
        top_boundary_head: f64,
        bottom_boundary_head: f64,
        left_boundary_head: Option<f64>,
        right_boundary_head: Option<f64>,
        base_conductivity: f64,
        heterogeneity: Option<&[f64]>,
        wells: &[WellPoint],
        origin_x: f64,
        origin_y: f64,
    ) -> FlowFieldResult {
        let n = self.rows * self.cols;
        let mut head = vec![0.0_f64; n];
        let mut conductivity = vec![base_conductivity; n];

        if let Some(het) = heterogeneity {
            for (i, &h) in het.iter().take(n).enumerate() {
                conductivity[i] = base_conductivity * h.max(0.1);
            }
        }

        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = r * self.cols + c;
                let frac_r = r as f64 / (self.rows - 1).max(1) as f64;
                let mut h = top_boundary_head + (bottom_boundary_head - top_boundary_head) * frac_r;

                if let Some(left_h) = left_boundary_head {
                    if let Some(right_h) = right_boundary_head {
                        let frac_c = c as f64 / (self.cols - 1).max(1) as f64;
                        let h_col = left_h + (right_h - left_h) * frac_c;
                        h = 0.5 * (h + h_col);
                    }
                }
                head[idx] = h;
            }
        }

        let mut converged = false;
        let mut iterations = 0;

        for iter in 0..self.max_iter {
            iterations = iter + 1;
            let mut max_delta = 0.0_f64;

            for r in 1..self.rows - 1 {
                for c in 1..self.cols - 1 {
                    let idx = r * self.cols + c;

                    let k_center = conductivity[idx];
                    let k_up = 0.5 * (k_center + conductivity[(r - 1) * self.cols + c]);
                    let k_down = 0.5 * (k_center + conductivity[(r + 1) * self.cols + c]);
                    let k_left = 0.5 * (k_center + conductivity[r * self.cols + (c - 1)]);
                    let k_right = 0.5 * (k_center + conductivity[r * self.cols + (c + 1)]);

                    let h_up = head[(r - 1) * self.cols + c];
                    let h_down = head[(r + 1) * self.cols + c];
                    let h_left = head[r * self.cols + (c - 1)];
                    let h_right = head[r * self.cols + (c + 1)];

                    let denominator = k_up + k_down + k_left + k_right;
                    if denominator < 1e-15 {
                        continue;
                    }

                    let mut new_h =
                        (k_up * h_up + k_down * h_down + k_left * h_left + k_right * h_right)
                            / denominator;

                    for w in wells {
                        if w.row == r && w.col == c {
                            let flux = w.discharge_rate_m3_d / 86400.0;
                            let cell_area = self.cell_size * self.cell_size;
                            let thickness = 5.0;
                            let porosity = 0.25;
                            new_h += flux / (k_center * cell_area / (self.cell_size)
                                * (1.0 - porosity)
                                * thickness
                                * 1e-6);
                        }
                    }

                    let delta = (new_h - head[idx]).abs();
                    max_delta = max_delta.max(delta);
                    head[idx] = new_h;
                }
            }

            for c in 0..self.cols {
                head[c] = top_boundary_head;
                head[(self.rows - 1) * self.cols + c] = bottom_boundary_head;
            }
            if let Some(lh) = left_boundary_head {
                for r in 0..self.rows {
                    head[r * self.cols] = lh;
                }
            }
            if let Some(rh) = right_boundary_head {
                for r in 0..self.rows {
                    head[r * self.cols + (self.cols - 1)] = rh;
                }
            }

            if max_delta < self.tol {
                converged = true;
                break;
            }
        }

        let mut grid = Vec::with_capacity(n);
        let mut avg_vel = 0.0_f64;
        let mut max_vel = 0.0_f64;
        let mut avg_head = 0.0_f64;
        let mut arrows = Vec::new();

        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = r * self.cols + c;
                let h = head[idx];
                let x = origin_x + c as f64 * self.cell_size;
                let y = origin_y + r as f64 * self.cell_size;

                let dh_dx = if c == 0 {
                    (head[idx + 1] - h) / self.cell_size
                } else if c == self.cols - 1 {
                    (h - head[idx - 1]) / self.cell_size
                } else {
                    (head[idx + 1] - head[idx - 1]) / (2.0 * self.cell_size)
                };

                let dh_dy = if r == 0 {
                    (head[idx + self.cols] - h) / self.cell_size
                } else if r == self.rows - 1 {
                    (h - head[idx - self.cols]) / self.cell_size
                } else {
                    (head[idx + self.cols] - head[idx - self.cols]) / (2.0 * self.cell_size)
                };

                let grad_mag = (dh_dx * dh_dx + dh_dy * dh_dy).sqrt();
                let k = conductivity[idx];
                let darcy_vel = k * grad_mag;
                let seepage_vel = darcy_vel / 0.25 * 86400.0;

                let direction = if grad_mag < 1e-9 {
                    0.0
                } else {
                    dh_dy.atan2(dh_dx).to_degrees()
                };

                avg_vel += seepage_vel;
                max_vel = max_vel.max(seepage_vel);
                avg_head += h;

                if r % 2 == 0 && c % 2 == 0 {
                    let arrow_len = (seepage_vel.log(10.0).max(0.0)) * self.cell_size * 0.5;
                    let dx = dh_dx / grad_mag.max(1e-9) * arrow_len;
                    let dy = dh_dy / grad_mag.max(1e-9) * arrow_len;
                    arrows.push(FlowArrow {
                        start_x: x,
                        start_y: y,
                        end_x: x + dx,
                        end_y: y + dy,
                        magnitude: seepage_vel,
                    });
                }

                grid.push(GridCell {
                    row: r,
                    col: c,
                    x,
                    y,
                    hydraulic_head_m: h,
                    hydraulic_conductivity_m_s: k,
                    porosity: 0.25,
                    thickness_m: 5.0,
                    flow_direction_deg: direction,
                    flow_velocity_m_d: seepage_vel,
                });
            }
        }

        let cells = n as f64;
        avg_vel /= cells;
        avg_head /= cells;

        let top_avg: f64 = (0..self.cols).map(|c| head[c]).sum::<f64>() / self.cols as f64;
        let bot_avg: f64 = (0..self.cols)
            .map(|c| head[(self.rows - 1) * self.cols + c])
            .sum::<f64>()
            / self.cols as f64;
        let total_distance = self.cell_size * (self.rows - 1) as f64;
        let gradient = (top_avg - bot_avg).abs() / total_distance.max(1.0);

        let travel_times = self.compute_travel_times(wells, &grid);

        FlowFieldResult {
            grid_rows: self.rows,
            grid_cols: self.cols,
            cell_size_m: self.cell_size,
            origin_x,
            origin_y,
            grid,
            wells: wells.to_vec(),
            avg_velocity_m_d: avg_vel,
            max_velocity_m_d: max_vel,
            avg_head_m: avg_head,
            head_gradient: gradient,
            darcy_flow_summary: arrows,
            travel_time_days: travel_times,
            convergence_status: converged,
            iterations,
        }
    }

    fn compute_travel_times(&self, wells: &[WellPoint], grid: &[GridCell]) -> Vec<TravelTimePoint> {
        let mut results = Vec::new();
        let source_wells: Vec<&WellPoint> = wells
            .iter()
            .filter(|w| matches!(w.well_type, WellType::ContaminationSource | WellType::Recharge))
            .collect();
        let monitor_wells: Vec<&WellPoint> = wells
            .iter()
            .filter(|w| matches!(w.well_type, WellType::Monitor))
            .collect();

        for src in &source_wells {
            for tgt in &monitor_wells {
                let src_idx = src.row * self.cols + src.col;
                let tgt_idx = tgt.row * self.cols + tgt.col;

                if src_idx >= grid.len() || tgt_idx >= grid.len() {
                    continue;
                }

                let dx = (grid[tgt_idx].x - grid[src_idx].x).abs();
                let dy = (grid[tgt_idx].y - grid[src_idx].y).abs();
                let dist = (dx * dx + dy * dy).sqrt();

                let path_samples = 20;
                let mut avg_vel_path = 0.0_f64;
                for s in 0..path_samples {
                    let frac = s as f64 / (path_samples - 1) as f64;
                    let r = (src.row as f64 + (tgt.row as f64 - src.row as f64) * frac) as usize;
                    let c = (src.col as f64 + (tgt.col as f64 - src.col as f64) * frac) as usize;
                    let idx = (r.min(self.rows - 1)) * self.cols + c.min(self.cols - 1);
                    avg_vel_path += grid[idx].flow_velocity_m_d;
                }
                avg_vel_path /= path_samples as f64;

                let travel_days = if avg_vel_path > 1e-6 {
                    dist / avg_vel_path
                } else {
                    f64::INFINITY
                };

                let quality = if travel_days.is_infinite() {
                    PathQuality::Trapped
                } else {
                    PathQuality::Direct
                };

                results.push(TravelTimePoint {
                    source_id: src.id.clone(),
                    target_id: tgt.id.clone(),
                    distance_m: dist,
                    travel_days,
                    path_quality: quality,
                });
            }
        }

        results
    }
}

pub fn default_simulation_params() -> (usize, usize, f64, Vec<WellPoint>) {
    let rows = 12;
    let cols = 16;
    let cell_size = 8.0;

    let wells = vec![
        WellPoint {
            id: "W-SRC-01".to_string(),
            row: 2,
            col: 4,
            x: 0.0 + 4.0 * cell_size,
            y: 0.0 + 2.0 * cell_size,
            well_type: WellType::ContaminationSource,
            discharge_rate_m3_d: 0.05,
            concentration_ppm: 350.0,
        },
        WellPoint {
            id: "W-SRC-02".to_string(),
            row: 1,
            col: 11,
            x: 0.0 + 11.0 * cell_size,
            y: 0.0 + 1.0 * cell_size,
            well_type: WellType::Recharge,
            discharge_rate_m3_d: 2.0,
            concentration_ppm: 20.0,
        },
        WellPoint {
            id: "W-MON-01".to_string(),
            row: 6,
            col: 7,
            x: 0.0 + 7.0 * cell_size,
            y: 0.0 + 6.0 * cell_size,
            well_type: WellType::Monitor,
            discharge_rate_m3_d: 0.0,
            concentration_ppm: 0.0,
        },
        WellPoint {
            id: "W-MON-02".to_string(),
            row: 9,
            col: 3,
            x: 0.0 + 3.0 * cell_size,
            y: 0.0 + 9.0 * cell_size,
            well_type: WellType::Monitor,
            discharge_rate_m3_d: 0.0,
            concentration_ppm: 0.0,
        },
        WellPoint {
            id: "W-MON-03".to_string(),
            row: 10,
            col: 13,
            x: 0.0 + 13.0 * cell_size,
            y: 0.0 + 10.0 * cell_size,
            well_type: WellType::Monitor,
            discharge_rate_m3_d: 0.0,
            concentration_ppm: 0.0,
        },
        WellPoint {
            id: "W-PUMP-01".to_string(),
            row: 5,
            col: 14,
            x: 0.0 + 14.0 * cell_size,
            y: 0.0 + 5.0 * cell_size,
            well_type: WellType::Pumping,
            discharge_rate_m3_d: -5.0,
            concentration_ppm: 0.0,
        },
    ];

    (rows, cols, cell_size, wells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_field_solve() {
        let (rows, cols, size, wells) = default_simulation_params();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            15.0,
            10.0,
            None,
            None,
            1e-5,
            None,
            &wells,
            0.0,
            0.0,
        );
        assert!(result.grid.len() == rows * cols);
        assert!(result.iterations > 0);
    }
}
