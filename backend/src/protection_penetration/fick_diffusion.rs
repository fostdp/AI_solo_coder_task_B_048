use serde::{Deserialize, Serialize};
use super::materials::ProtectiveMaterial;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenetrationProfile {
    pub depth_um: f64,
    pub concentration_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenetrationResult {
    pub material_name: String,
    pub temperature_c: f64,
    pub relative_humidity: f64,
    pub porosity: f64,
    pub surface_concentration: f64,
    pub total_time_seconds: f64,
    pub total_time_hours: f64,
    pub effective_diffusion_coeff: f64,
    pub max_penetration_um: f64,
    pub average_penetration_um: f64,
    pub concentration_front_um: f64,
    pub profile: Vec<PenetrationProfile>,
    pub time_series: Vec<PenetrationTimePoint>,
    pub protection_efficiency: f64,
    pub estimated_lifetime_years: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenetrationTimePoint {
    pub time_hours: f64,
    pub avg_penetration_um: f64,
    pub max_penetration_um: f64,
    pub surface_concentration_ratio: f64,
}

pub struct PenetrationSimulator {
    dx_um: f64,
    max_depth_um: f64,
    tolerance: f64,
}

impl PenetrationSimulator {
    pub fn new() -> Self {
        Self {
            dx_um: 1.0,
            max_depth_um: 1000.0,
            tolerance: 1e-6,
        }
    }

    pub fn simulate(
        &self,
        material: &ProtectiveMaterial,
        temperature_c: f64,
        relative_humidity: f64,
        porosity: f64,
        surface_concentration: f64,
        time_hours: f64,
        substrate_roughness: Option<f64>,
    ) -> PenetrationResult {
        let total_time_seconds = time_hours * 3600.0;
        let effective_d = self.calc_effective_diffusion_coeff(
            material.diffusion_coefficient,
            temperature_c,
            material.optimal_temp,
            porosity,
            relative_humidity,
            substrate_roughness,
        );

        let num_points = (self.max_depth_um / self.dx_um) as usize;
        let mut concentration = vec![0.0_f64; num_points];
        concentration[0] = surface_concentration;

        let dt = self.calc_stable_dt(effective_d, self.dx_um * 1e-6);
        let num_steps = (total_time_seconds / dt) as usize;

        let sample_steps = 50;
        let step_interval = (num_steps / sample_steps).max(1);
        let mut time_series = Vec::with_capacity(sample_steps + 1);

        for step in 0..num_steps {
            self.fick_2d_step(&mut concentration, effective_d, dt, self.dx_um * 1e-6, surface_concentration);

            if step % step_interval == 0 || step == num_steps - 1 {
                let t_hours = (step as f64 * dt) / 3600.0;
                let (avg_p, max_p, surf_r) = self.summary(&concentration, surface_concentration);
                time_series.push(PenetrationTimePoint {
                    time_hours: t_hours,
                    avg_penetration_um: avg_p,
                    max_penetration_um: max_p,
                    surface_concentration_ratio: surf_r,
                });
            }
        }

        let threshold = surface_concentration * 0.01;
        let max_penetration = self.find_front(&concentration, threshold, self.dx_um);
        let avg_penetration = self.find_front(&concentration, surface_concentration * 0.5, self.dx_um);
        let conc_front = self.find_front(&concentration, surface_concentration * 0.001, self.dx_um);

        let profile: Vec<PenetrationProfile> = concentration
            .iter()
            .enumerate()
            .step_by(5)
            .map(|(i, c)| PenetrationProfile {
                depth_um: i as f64 * self.dx_um,
                concentration_ratio: c / surface_concentration.max(1e-9),
            })
            .collect();

        let protection_efficiency = self.calc_protection_efficiency(
            &concentration,
            surface_concentration,
            material,
            porosity,
        );

        let lifetime = self.estimate_lifetime(max_penetration, material, effective_d, porosity);

        PenetrationResult {
            material_name: material.name.as_str().to_string(),
            temperature_c,
            relative_humidity,
            porosity,
            surface_concentration,
            total_time_seconds,
            total_time_hours: time_hours,
            effective_diffusion_coeff: effective_d,
            max_penetration_um: max_penetration,
            average_penetration_um: avg_penetration,
            concentration_front_um: conc_front,
            profile,
            time_series,
            protection_efficiency,
            estimated_lifetime_years: lifetime,
        }
    }

    fn calc_effective_diffusion_coeff(
        &self,
        d0: f64,
        temp_c: f64,
        optimal_temp: f64,
        porosity: f64,
        rh: f64,
        roughness: Option<f64>,
    ) -> f64 {
        let temp_k = temp_c + 273.15;
        let optimal_k = optimal_temp + 273.15;
        let ea = 25000.0;
        let r = 8.314;

        let arrhenius = (-ea / r * (1.0 / temp_k - 1.0 / optimal_k)).exp();

        let porosity_factor = porosity.powf(1.5) * (2.0 - porosity);

        let rh_factor = 1.0 - 0.5 * (rh / 100.0).powi(2);

        let roughness_factor = match roughness {
            Some(r) if r > 1.0 => 1.0 + 0.3 * (r - 1.0).min(3.0),
            _ => 1.0,
        };

        d0 * arrhenius * porosity_factor * rh_factor * roughness_factor
    }

    fn calc_stable_dt(&self, d: f64, dx: f64) -> f64 {
        let fourier_max = 0.45;
        fourier_max * dx * dx / d
    }

    fn fick_2d_step(
        &self,
        c: &mut Vec<f64>,
        d: f64,
        dt: f64,
        dx: f64,
        surface_c: f64,
    ) {
        let n = c.len();
        let mut new_c = vec![0.0_f64; n];

        new_c[0] = surface_c;

        for i in 1..n - 1 {
            let d2c_dx2 = (c[i + 1] - 2.0 * c[i] + c[i - 1]) / (dx * dx);
            new_c[i] = c[i] + d * dt * d2c_dx2;
        }

        if n >= 2 {
            new_c[n - 1] = c[n - 1] + d * dt * (c[n - 2] - c[n - 1]) / (dx * dx);
        }

        for i in 0..n {
            c[i] = new_c[i];
        }
    }

    fn find_front(&self, c: &[f64], threshold: f64, dx_um: f64) -> f64 {
        for i in (1..c.len()).rev() {
            if c[i] >= threshold {
                let exact = if i < c.len() - 1 {
                    let c0 = c[i];
                    let c1 = c[i + 1];
                    if (c0 - c1).abs() > 1e-12 {
                        let frac = (c0 - threshold) / (c0 - c1);
                        (i as f64 + frac) * dx_um
                    } else {
                        i as f64 * dx_um
                    }
                } else {
                    i as f64 * dx_um
                };
                return exact;
            }
        }
        0.0
    }

    fn summary(&self, c: &[f64], surface_c: f64) -> (f64, f64, f64) {
        let threshold_50 = surface_c * 0.5;
        let threshold_max = surface_c * 0.01;

        let mut avg_front = 0.0;
        let mut max_front = 0.0;

        for i in (1..c.len()).rev() {
            if avg_front == 0.0 && c[i] >= threshold_50 {
                avg_front = i as f64 * self.dx_um;
            }
            if max_front == 0.0 && c[i] >= threshold_max {
                max_front = i as f64 * self.dx_um;
            }
            if avg_front > 0.0 && max_front > 0.0 {
                break;
            }
        }

        let surf_ratio = if surface_c > 0.0 { c[0] / surface_c } else { 1.0 };
        (avg_front, max_front, surf_ratio)
    }

    fn calc_protection_efficiency(
        &self,
        c: &[f64],
        surface_c: f64,
        material: &ProtectiveMaterial,
        porosity: f64,
    ) -> f64 {
        let total_uptake: f64 = c.iter().sum::<f64>() * self.dx_um;
        let max_theoretical = surface_c * self.max_depth_um;
        let uptake_ratio = total_uptake / max_theoretical.max(1e-9);

        let material_factor = match material.name {
            super::materials::MaterialType::Silicone => 0.85,
            super::materials::MaterialType::Fluoropolymer => 0.95,
            super::materials::MaterialType::Acrylate => 0.75,
            super::materials::MaterialType::Epoxy => 0.90,
            super::materials::MaterialType::Paraffin => 0.65,
            super::materials::MaterialType::NanoSiO2 => 0.92,
        };

        let porosity_penalty = 1.0 - 0.5 * porosity.min(0.5);

        (uptake_ratio * 0.4 + material_factor * 0.4 + porosity_penalty * 0.2).clamp(0.0, 1.0)
    }

    fn estimate_lifetime(
        &self,
        penetration_um: f64,
        material: &ProtectiveMaterial,
        _effective_d: f64,
        porosity: f64,
    ) -> f64 {
        let base_life = match material.name {
            super::materials::MaterialType::Silicone => 15.0,
            super::materials::MaterialType::Fluoropolymer => 30.0,
            super::materials::MaterialType::Acrylate => 8.0,
            super::materials::MaterialType::Epoxy => 25.0,
            super::materials::MaterialType::Paraffin => 3.0,
            super::materials::MaterialType::NanoSiO2 => 20.0,
        };

        let penetration_factor = if penetration_um < 50.0 {
            0.5
        } else if penetration_um < 100.0 {
            0.8
        } else if penetration_um < 300.0 {
            1.0
        } else {
            0.9
        };

        let porosity_factor = 1.0 - 0.6 * porosity.min(0.5);

        base_life * penetration_factor * porosity_factor
    }
}

impl Default for PenetrationSimulator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn analytical_penetration(
    diffusion_coeff: f64,
    time_seconds: f64,
) -> f64 {
    use std::f64::consts::SQRT_2;
    let erf_inv_99 = 1.82138636;
    SQRT_2 * erf_inv_99 * (2.0 * diffusion_coeff * time_seconds).sqrt() * 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_penetration_simulation() {
        let sim = PenetrationSimulator::new();
        let mat = silicone_standard();
        let result = sim.simulate(&mat, 20.0, 50.0, 0.15, 1.0, 24.0, None);
        assert!(result.max_penetration_um > 0.0);
        assert!(result.effective_diffusion_coeff > 0.0);
    }

    fn silicone_standard() -> ProtectiveMaterial {
        super::materials::silicone_standard()
    }

    #[test]
    fn test_analytical() {
        let d = 5e-10;
        let t = 3600.0;
        let p = analytical_penetration(d, t);
        assert!(p > 0.0);
    }
}
