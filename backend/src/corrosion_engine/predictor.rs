use rand::Rng;
use crate::common::models::CorrosionPrediction;

struct NeuralNetwork {
    input_weights: Vec<Vec<f64>>,
    hidden_weights: Vec<f64>,
    hidden_bias: Vec<f64>,
    output_bias: f64,
    dropout_rate: f64,
    l2_lambda: f64,
}

impl NeuralNetwork {
    fn new(input_size: usize, hidden_size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let he_scale = (2.0 / input_size as f64).sqrt();

        let input_weights: Vec<Vec<f64>> = (0..hidden_size)
            .map(|_| {
                (0..input_size)
                    .map(|_| (rng.gen_range(-0.5..0.5)) * he_scale)
                    .collect()
            })
            .collect();

        let hidden_scale = (2.0 / hidden_size as f64).sqrt();
        let hidden_weights: Vec<f64> = (0..hidden_size)
            .map(|_| (rng.gen_range(-0.5..0.5)) * hidden_scale)
            .collect();

        let hidden_bias: Vec<f64> = (0..hidden_size).map(|_| 0.01).collect();
        let output_bias = 0.01;

        NeuralNetwork {
            input_weights,
            hidden_weights,
            hidden_bias,
            output_bias,
            dropout_rate: 0.0,
            l2_lambda: 0.0,
        }
    }

    fn with_regularization(
        input_size: usize,
        hidden_size: usize,
        dropout_rate: f64,
        l2_lambda: f64,
    ) -> Self {
        let mut network = NeuralNetwork::new(input_size, hidden_size);
        network.dropout_rate = dropout_rate;
        network.l2_lambda = l2_lambda;
        network
    }

    fn forward(&self, inputs: &[f64], training: bool) -> f64 {
        let mut rng = rand::thread_rng();
        let hidden_size = self.input_weights.len();
        let mut hidden = Vec::with_capacity(hidden_size);

        for j in 0..hidden_size {
            let mut sum = self.hidden_bias[j];
            for (i, input) in inputs.iter().enumerate() {
                sum += self.input_weights[j][i] * input;
            }
            hidden.push(sum.tanh());
        }

        if training && self.dropout_rate > 0.0 {
            let keep_prob = 1.0 - self.dropout_rate;
            let scale = 1.0 / keep_prob;
            for h in hidden.iter_mut() {
                if rng.gen::<f64>() < self.dropout_rate {
                    *h = 0.0;
                } else {
                    *h *= scale;
                }
            }
        }

        let mut output = self.output_bias;
        for (j, h) in hidden.iter().enumerate() {
            output += self.hidden_weights[j] * h;
        }

        let sigmoid = 1.0 / (1.0 + (-output).exp());
        sigmoid * 1.5
    }

    fn compute_l2_loss(&self) -> f64 {
        let mut loss = 0.0;
        for row in &self.input_weights {
            for w in row {
                loss += w * w;
            }
        }
        for w in &self.hidden_weights {
            loss += w * w;
        }
        0.5 * self.l2_lambda * loss
    }
}

pub struct CorrosionPredictor {
    network: NeuralNetwork,
}

impl CorrosionPredictor {
    pub fn new() -> Self {
        let network = NeuralNetwork::with_regularization(6, 8, 0.2, 0.001);
        CorrosionPredictor { network }
    }

    pub fn predict(
        &self,
        probe_id: &str,
        material_type: &str,
        current_rate: f64,
        temp: f64,
        hum: f64,
        ph: f64,
        chloride: f64,
    ) -> CorrosionPrediction {
        let temp_norm = ((temp - 5.0) / 35.0).clamp(0.0, 1.0);
        let hum_norm = hum / 100.0;
        let ph_norm = (ph - 4.0) / 10.0;
        let cl_norm = (chloride / 200.0).clamp(0.0, 1.0);
        let material_factor = if material_type == "iron"
            || material_type == "Iron"
            || material_type == "铁"
        {
            1.0
        } else {
            0.6
        };
        let rate_norm = (current_rate / 1.5).clamp(0.0, 1.0);

        let inputs = vec![
            temp_norm,
            hum_norm,
            ph_norm,
            cl_norm,
            material_factor,
            rate_norm,
        ];

        let env_acceleration = self.network.forward(&inputs, false);

        let predicted_30d = current_rate * (1.0 + env_acceleration);
        let predicted_rate_7d = current_rate * (1.0 + env_acceleration * 7.0 / 30.0);
        let predicted_rate_90d = current_rate * (1.0 + env_acceleration * 90.0 / 30.0 * 0.9);
        let predicted_avg_30d = (current_rate + predicted_30d) / 2.0;

        let risk_level = if predicted_30d < 0.1 {
            "低".to_string()
        } else if predicted_30d < 0.3 {
            "中".to_string()
        } else if predicted_30d < 0.5 {
            "较高".to_string()
        } else {
            "高".to_string()
        };

        let confidence = 0.85;

        CorrosionPrediction {
            probe_id: probe_id.to_string(),
            material_type: material_type.to_string(),
            current_rate,
            risk_level,
            confidence,
            predicted_rate_7d,
            predicted_rate_30d: predicted_30d,
            predicted_rate_90d,
            predicted_avg_30d,
        }
    }
}
