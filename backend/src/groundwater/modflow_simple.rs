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

    // ─── 流场求解收敛性验证 ───

    #[test]
    fn test_flow_field_solves_and_converges() {
        let (rows, cols, size, wells) = default_simulation_params();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            15.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );
        assert_eq!(result.grid.len(), rows * cols);
        assert!(result.iterations > 0);
        assert!(result.convergence_status, "求解应收敛");
    }

    // ─── 流场方向验证（与手动计算一致） ───

    #[test]
    fn test_flow_direction_top_to_bottom() {
        // 顶部水头15m，底部10m → 流向应向下（y减小方向）
        let rows = 8;
        let cols = 8;
        let size = 10.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            15.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );

        // 取中间列，验证水头随行号增大而减小
        let col = cols / 2;
        let mut prev_head = f64::INFINITY;
        for row in 0..rows {
            let idx = row * cols + col;
            let h = result.grid[idx].hydraulic_head_m;
            assert!(h <= prev_head + 1e-3,
                "水头应沿流向递减: row={} h={}, prev={}", row, h, prev_head);
            prev_head = h;
        }
    }

    #[test]
    fn test_flow_direction_left_to_right() {
        // 左侧高水头，右侧低水头 → 流向向右
        let rows = 8;
        let cols = 10;
        let size = 10.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            12.0, 12.0, Some(15.0), Some(8.0), 1e-5, None, &wells, 0.0, 0.0,
        );

        // 取中间行，验证水头随列数增大而减小
        let row = rows / 2;
        let mut prev_head = f64::INFINITY;
        for col in 0..cols {
            let idx = row * cols + col;
            let h = result.grid[idx].hydraulic_head_m;
            assert!(h <= prev_head + 1e-2,
                "水头应从左向右递减: col={} h={}, prev={}", col, h, prev_head);
            prev_head = h;
        }
    }

    #[test]
    fn test_uniform_head_no_flow() {
        // 四周水头相同 → 几乎无流动
        let rows = 6;
        let cols = 6;
        let size = 10.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            10.0, 10.0, Some(10.0), Some(10.0), 1e-5, None, &wells, 0.0, 0.0,
        );

        // 所有单元水头应接近10m
        for cell in &result.grid {
            assert!((cell.hydraulic_head_m - 10.0).abs() < 0.5,
                "均匀水头应接近10m: ({},{}) = {}",
                cell.row, cell.col, cell.hydraulic_head_m);
        }
    }

    // ─── 井的影响验证 ───

    #[test]
    fn test_pumping_well_lowers_head() {
        let rows = 8;
        let cols = 8;
        let size = 10.0;
        let wells = vec![
            WellPoint {
                id: "P1".to_string(),
                row: 4, col: 4, x: 40.0, y: 40.0,
                well_type: WellType::Pumping,
                discharge_rate_m3_d: -50.0,
                concentration_ppm: 0.0,
            }
        ];
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            10.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );

        let pump_idx = 4 * cols + 4;
        let pump_head = result.grid[pump_idx].hydraulic_head_m;
        let far_idx = 0 * cols + 0;
        let far_head = result.grid[far_idx].hydraulic_head_m;

        assert!(pump_head < far_head,
            "抽水井处水头应低于远处: pump={}, far={}", pump_head, far_head);
    }

    #[test]
    fn test_recharge_well_raises_head() {
        let rows = 8;
        let cols = 8;
        let size = 10.0;
        let wells = vec![
            WellPoint {
                id: "R1".to_string(),
                row: 4, col: 4, x: 40.0, y: 40.0,
                well_type: WellType::Recharge,
                discharge_rate_m3_d: 30.0,
                concentration_ppm: 0.0,
            }
        ];
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            10.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );

        let recharge_idx = 4 * cols + 4;
        let recharge_head = result.grid[recharge_idx].hydraulic_head_m;
        let far_idx = 0 * cols + 0;
        let far_head = result.grid[far_idx].hydraulic_head_m;

        assert!(recharge_head > far_head,
            "补给井处水头应高于远处: recharge={}, far={}", recharge_head, far_head);
    }

    // ─── 网格与坐标一致性 ───

    #[test]
    fn test_grid_coordinates_origin() {
        let rows = 5;
        let cols = 5;
        let size = 10.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            10.0, 10.0, None, None, 1e-5, None, &wells, 5.0, 10.0,
        );

        let first = &result.grid[0];
        assert_eq!(first.row, 0);
        assert_eq!(first.col, 0);
        assert!((first.x - 5.0).abs() < 1e-6, "origin_x应作为网格起点");
        assert!((first.y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_grid_cell_count_matches() {
        let rows = 10;
        let cols = 12;
        let size = 5.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            10.0, 8.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );
        assert_eq!(result.grid.len(), rows * cols);
        assert_eq!(result.grid_rows, rows);
        assert_eq!(result.grid_cols, cols);
        assert_eq!(result.cell_size_m, size);
    }

    // ─── 流速与水力梯度一致性 ───

    #[test]
    fn test_velocity_positive_with_gradient() {
        let (rows, cols, size, wells) = default_simulation_params();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            15.0, 8.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );

        // 有梯度就应有流速
        assert!(result.avg_velocity_m_d > 0.0, "存在水力梯度时平均流速应为正");
        assert!(result.max_velocity_m_d >= result.avg_velocity_m_d);
        assert!(result.head_gradient > 0.0);
    }

    // ─── 边界/异常输入处理 ───

    #[test]
    fn test_zero_conductivity_still_solves() {
        let rows = 5;
        let cols = 5;
        let size = 10.0;
        let wells = Vec::new();
        let model = GroundwaterModel::new(rows, cols, size);
        // 极低渗透系数也应能求解
        let result = model.solve_steady_state(
            10.0, 8.0, None, None, 1e-10, None, &wells, 0.0, 0.0,
        );
        assert_eq!(result.grid.len(), rows * cols);
        assert!(result.convergence_status);
    }

    #[test]
    fn test_small_grid_solves() {
        // 最小网格也能工作
        let model = GroundwaterModel::new(3, 3, 1.0);
        let wells = Vec::new();
        let result = model.solve_steady_state(
            10.0, 5.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );
        assert_eq!(result.grid.len(), 9);
        assert!(result.iterations > 0);
    }

    // ─── 平均水头合理性 ───

    #[test]
    fn test_avg_head_within_bounds() {
        let (rows, cols, size, wells) = default_simulation_params();
        let model = GroundwaterModel::new(rows, cols, size);
        let result = model.solve_steady_state(
            15.0, 10.0, None, None, 1e-5, None, &wells, 0.0, 0.0,
        );
        assert!(result.avg_head_m > 10.0 && result.avg_head_m < 15.0,
            "平均水头应在上下边界之间: {}", result.avg_head_m);
    }

    // ─── 辅助函数：默认模拟参数 ───

    fn default_simulation_params() -> (usize, usize, f64, Vec<WellPoint>) {
        super::default_simulation_params()
    }
}
