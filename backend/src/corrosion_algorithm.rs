use rand::Rng;
use crate::models::{CorrosionPrediction, StabilityAssessment};

const B: f64 = 0.026;
const IRON_DENSITY: f64 = 7.87;
const IRON_ATOMIC_WEIGHT: f64 = 55.85;
const IRON_VALENCE: f64 = 2.0;
const COPPER_DENSITY: f64 = 8.96;
const COPPER_ATOMIC_WEIGHT: f64 = 63.55;
const COPPER_VALENCE: f64 = 2.0;
const FARADAY: f64 = 96485.0;
const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;

pub fn calculate_corrosion_rate_lpr(
    polarization_resistance: f64,
    material_type: &str,
) -> f64 {
    let rp = polarization_resistance.max(10.0);
    let icorr = B / rp;

    let (density, atomic_weight, valence) = match material_type {
        "copper" => (COPPER_DENSITY, COPPER_ATOMIC_WEIGHT, COPPER_VALENCE),
        _ => (IRON_DENSITY, IRON_ATOMIC_WEIGHT, IRON_VALENCE),
    };

    let corrosion_rate_mmpy = (3.27e-3 * icorr * atomic_weight * SECONDS_PER_YEAR)
        / (valence * FARADAY * density * 1e-3);

    corrosion_rate_mmpy.max(0.0001)
}

struct NeuralNetwork {
    input_weights: Vec<Vec<f64>>,
    hidden_weights: Vec<Vec<f64>>,
    hidden_bias: Vec<f64>,
    output_bias: f64,
    dropout_rate: f64,
    l2_lambda: f64,
}

impl NeuralNetwork {
    fn new(input_size: usize, hidden_size: usize) -> Self {
        Self::with_regularization(input_size, hidden_size, 0.2, 0.001)
    }

    fn with_regularization(
        input_size: usize,
        hidden_size: usize,
        dropout_rate: f64,
        l2_lambda: f64,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let scale = (2.0 / input_size as f64).sqrt();
        let mut input_weights = Vec::with_capacity(hidden_size);
        for _ in 0..hidden_size {
            let mut row = Vec::with_capacity(input_size);
            for _ in 0..input_size {
                row.push(rng.gen_range(-0.5..0.5) * scale);
            }
            input_weights.push(row);
        }

        let hidden_scale = (2.0 / hidden_size as f64).sqrt();
        let mut hidden_weights = Vec::with_capacity(hidden_size);
        for _ in 0..hidden_size {
            hidden_weights.push(vec![rng.gen_range(-0.3..0.3) * hidden_scale]);
        }

        let hidden_bias: Vec<f64> = (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect();
        let output_bias = rng.gen_range(-0.05..0.05);

        NeuralNetwork {
            input_weights,
            hidden_weights,
            hidden_bias,
            output_bias,
            dropout_rate,
            l2_lambda,
        }
    }

    fn forward(&self, inputs: &[f64], training: bool) -> f64 {
        let mut hidden = vec![0.0; self.hidden_bias.len()];
        let mut rng = rand::thread_rng();

        for i in 0..self.hidden_bias.len() {
            let mut sum = self.hidden_bias[i];
            for j in 0..inputs.len() {
                sum += self.input_weights[i][j] * inputs[j];
            }
            hidden[i] = Self::tanh(sum);

            if training && rng.gen::<f64>() < self.dropout_rate {
                hidden[i] = 0.0;
            }
        }

        let scale = if training { 1.0 / (1.0 - self.dropout_rate) } else { 1.0 };

        let mut output = self.output_bias;
        for i in 0..hidden.len() {
            output += hidden[i] * self.hidden_weights[i][0] * scale;
        }
        Self::sigmoid(output) * 1.5
    }

    fn tanh(x: f64) -> f64 {
        x.tanh()
    }

    fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }

    fn compute_l2_loss(&self) -> f64 {
        let mut loss = 0.0;
        for row in &self.input_weights {
            for w in row {
                loss += w * w;
            }
        }
        for row in &self.hidden_weights {
            for w in row {
                loss += w * w;
            }
        }
        self.l2_lambda * loss
    }
}

pub struct CorrosionPredictor {
    network: NeuralNetwork,
}

impl CorrosionPredictor {
    pub fn new() -> Self {
        CorrosionPredictor {
            network: NeuralNetwork::new(6, 8),
        }
    }

    pub fn predict(
        &self,
        probe_id: &str,
        material_type: &str,
        current_rate: f64,
        temperature: f64,
        humidity: f64,
        ph: f64,
        chloride: f64,
    ) -> CorrosionPrediction {
        let temp_norm = (temperature - 10.0) / 30.0;
        let hum_norm = humidity / 100.0;
        let ph_norm = (ph - 4.0) / 8.0;
        let cl_norm = (chloride / 200.0).min(1.0);
        let material_factor = if material_type == "iron" { 1.0 } else { 0.6 };
        let rate_norm = (current_rate / 1.0).min(1.0);

        let inputs = vec![temp_norm, hum_norm, ph_norm, cl_norm, material_factor, rate_norm];

        let env_acceleration = self.network.forward(&inputs, false);

        let predicted_rate_7d = current_rate * (1.0 + env_acceleration * 0.15);
        let predicted_rate_30d = current_rate * (1.0 + env_acceleration * 0.45);
        let predicted_rate_90d = current_rate * (1.0 + env_acceleration * 0.90);

        let avg_predicted = (predicted_rate_7d + predicted_rate_30d + predicted_rate_90d) / 3.0;
        let (risk_level, confidence) = if avg_predicted > 0.7 {
            ("严重".to_string(), 0.92)
        } else if avg_predicted > 0.5 {
            ("高".to_string(), 0.85)
        } else if avg_predicted > 0.3 {
            ("中等".to_string(), 0.78)
        } else if avg_predicted > 0.15 {
            ("低".to_string(), 0.70)
        } else {
            ("轻微".to_string(), 0.65)
        };

        CorrosionPrediction {
            probe_id: probe_id.to_string(),
            material_type: material_type.to_string(),
            current_rate,
            predicted_rate_7d: predicted_rate_7d.max(0.0001),
            predicted_rate_30d: predicted_rate_30d.max(0.0001),
            predicted_rate_90d: predicted_rate_90d.max(0.0001),
            risk_level,
            confidence,
        }
    }
}

impl Default for CorrosionPredictor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StabilityAnalyzer;

impl StabilityAnalyzer {
    pub fn assess(
        probe_id: &str,
        material_type: &str,
        corrosion_rate: f64,
        temperature: f64,
        humidity: f64,
        ph: f64,
        chloride: f64,
    ) -> StabilityAssessment {
        let env_score = Self::calculate_environment_score(temperature, humidity, ph, chloride);
        let material_factor = if material_type == "iron" { 1.0 } else { 0.75 };

        let corrosion_rate_norm = (corrosion_rate / 1.0).min(1.0);
        let stability_index = (env_score - corrosion_rate_norm * material_factor * 0.6).max(0.0).min(1.0);

        let (stability_level, remaining_lifetime) = if stability_index >= 0.85 {
            ("极稳定".to_string(), 200.0)
        } else if stability_index >= 0.70 {
            ("稳定".to_string(), 100.0)
        } else if stability_index >= 0.50 {
            ("较稳定".to_string(), 50.0)
        } else if stability_index >= 0.30 {
            ("不稳定".to_string(), 20.0)
        } else {
            ("极不稳定".to_string(), 5.0)
        };

        let adjusted_lifetime = remaining_lifetime * (0.5 / corrosion_rate.max(0.1));

        let mut recommendations = Vec::new();

        if temperature > 25.0 {
            recommendations.push("建议加强通风降温，避免高温加速腐蚀".to_string());
        }
        if humidity > 70.0 {
            recommendations.push("建议安装除湿设备，控制土壤湿度在40%-60%".to_string());
        }
        if ph < 5.5 {
            recommendations.push("土壤酸性较强，建议施加石灰中和".to_string());
        } else if ph > 8.5 {
            recommendations.push("土壤碱性较强，建议施加石膏调节".to_string());
        }
        if chloride > 80.0 {
            recommendations.push("氯离子含量过高，建议进行土壤脱盐处理".to_string());
        }
        if corrosion_rate > 0.5 {
            recommendations.push(format!(
                "{}腐蚀速率超标，建议立即采取保护措施",
                if material_type == "iron" { "铁器" } else { "铜器" }
            ));
        }
        if material_type == "iron" && stability_index < 0.5 {
            recommendations.push("铁器建议进行缓蚀处理和密封封存".to_string());
        }
        if recommendations.is_empty() {
            recommendations.push("环境条件良好，保持常规监测即可".to_string());
        }

        StabilityAssessment {
            probe_id: probe_id.to_string(),
            material_type: material_type.to_string(),
            stability_index,
            stability_level,
            remaining_lifetime_years: adjusted_lifetime.max(0.5),
            recommendations,
        }
    }

    fn calculate_environment_score(
        temperature: f64,
        humidity: f64,
        ph: f64,
        chloride: f64,
    ) -> f64 {
        let temp_score = if (10.0..=20.0).contains(&temperature) {
            1.0
        } else if temperature < 10.0 {
            0.7 + (temperature / 10.0) * 0.3
        } else {
            (1.0 - ((temperature - 20.0) / 30.0) * 0.5).max(0.3)
        };

        let hum_score = if (40.0..=60.0).contains(&humidity) {
            1.0
        } else if humidity < 40.0 {
            0.5 + (humidity / 40.0) * 0.5
        } else {
            (1.0 - ((humidity - 60.0) / 60.0) * 0.7).max(0.2)
        };

        let ph_score = if (6.0..=8.0).contains(&ph) {
            1.0
        } else if ph < 6.0 {
            0.3 + (ph / 6.0) * 0.7
        } else {
            (1.0 - ((ph - 8.0) / 6.0) * 0.6).max(0.3)
        };

        let cl_score = if chloride <= 50.0 {
            1.0
        } else if chloride <= 150.0 {
            1.0 - ((chloride - 50.0) / 100.0) * 0.7
        } else {
            0.2
        };

        (temp_score * 0.2 + hum_score * 0.3 + ph_score * 0.2 + cl_score * 0.3)
    }
}
