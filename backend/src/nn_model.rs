use rand::Rng;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    pub inputs: Vec<f64>,
    pub target: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingHistory {
    pub epochs: usize,
    pub train_losses: Vec<f64>,
    pub val_losses: Vec<f64>,
    pub best_val_loss: f64,
    pub best_epoch: usize,
    pub stopped_early: bool,
    pub patience_used: usize,
}

#[derive(Debug, Clone)]
pub struct TrainableNeuralNetwork {
    pub input_weights: Vec<Vec<f64>>,
    pub hidden_weights: Vec<Vec<f64>>,
    pub hidden_bias: Vec<f64>,
    pub output_bias: f64,
    pub dropout_rate: f64,
    pub l2_lambda: f64,
    pub learning_rate: f64,
    input_size: usize,
    hidden_size: usize,
}

impl TrainableNeuralNetwork {
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        Self::with_hyperparams(input_size, hidden_size, 0.2, 0.001, 0.005)
    }

    pub fn with_hyperparams(
        input_size: usize,
        hidden_size: usize,
        dropout_rate: f64,
        l2_lambda: f64,
        learning_rate: f64,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let he_scale = (2.0 / input_size as f64).sqrt();

        let mut input_weights = Vec::with_capacity(hidden_size);
        for _ in 0..hidden_size {
            let mut row = Vec::with_capacity(input_size);
            for _ in 0..input_size {
                row.push(rng.gen_range(-1.0..1.0) * he_scale);
            }
            input_weights.push(row);
        }

        let hidden_scale = (2.0 / hidden_size as f64).sqrt();
        let mut hidden_weights = Vec::with_capacity(hidden_size);
        for _ in 0..hidden_size {
            hidden_weights.push(vec![rng.gen_range(-1.0..1.0) * hidden_scale]);
        }

        let hidden_bias: Vec<f64> = (0..hidden_size).map(|_| 0.0).collect();
        let output_bias = 0.0;

        TrainableNeuralNetwork {
            input_weights,
            hidden_weights,
            hidden_bias,
            output_bias,
            dropout_rate,
            l2_lambda,
            learning_rate,
            input_size,
            hidden_size,
        }
    }

    pub fn forward(&self, inputs: &[f64], training: bool) -> (f64, Vec<f64>, Vec<f64>) {
        assert_eq!(inputs.len(), self.input_size);
        let mut rng = rand::thread_rng();

        let mut z_hidden = vec![0.0; self.hidden_size];
        let mut a_hidden = vec![0.0; self.hidden_size];
        let mut dropout_mask = vec![1.0; self.hidden_size];

        for i in 0..self.hidden_size {
            let mut sum = self.hidden_bias[i];
            for j in 0..self.input_size {
                sum += self.input_weights[i][j] * inputs[j];
            }
            z_hidden[i] = sum;
            a_hidden[i] = Self::tanh(sum);

            if training && rng.gen::<f64>() < self.dropout_rate {
                dropout_mask[i] = 0.0;
                a_hidden[i] = 0.0;
            }
        }

        let scale = if training { 1.0 / (1.0 - self.dropout_rate) } else { 1.0 };

        let mut z_output = self.output_bias;
        for i in 0..self.hidden_size {
            z_output += a_hidden[i] * self.hidden_weights[i][0] * scale;
        }
        let output = Self::sigmoid(z_output) * 1.5;

        (output, z_hidden, a_hidden)
    }

    fn tanh(x: f64) -> f64 {
        x.tanh()
    }

    fn tanh_derivative(x: f64) -> f64 {
        let t = x.tanh();
        1.0 - t * t
    }

    fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }

    fn sigmoid_derivative(x: f64) -> f64 {
        let s = Self::sigmoid(x);
        s * (1.0 - s)
    }

    pub fn train_batch(
        &mut self,
        batch: &[TrainingSample],
    ) -> f64 {
        let mut grad_input_w = vec![vec![0.0; self.input_size]; self.hidden_size];
        let mut grad_hidden_w = vec![vec![0.0; 1]; self.hidden_size];
        let mut grad_hidden_b = vec![0.0; self.hidden_size];
        let mut grad_output_b = 0.0;
        let mut total_loss = 0.0;

        for sample in batch {
            let (output, z_hidden, a_hidden) = self.forward(&sample.inputs, true);

            let error = output - sample.target;
            total_loss += error * error * 0.5;

            let delta_output = error * Self::sigmoid_derivative(output / 1.5) * 1.5;
            grad_output_b += delta_output;

            let mut delta_hidden = vec![0.0; self.hidden_size];
            for i in 0..self.hidden_size {
                delta_hidden[i] = delta_output * self.hidden_weights[i][0] * Self::tanh_derivative(z_hidden[i]);
            }

            for i in 0..self.hidden_size {
                grad_hidden_w[i][0] += delta_output * a_hidden[i];
                grad_hidden_b[i] += delta_hidden[i];
                for j in 0..self.input_size {
                    grad_input_w[i][j] += delta_hidden[i] * sample.inputs[j];
                }
            }
        }

        let batch_size = batch.len() as f64;
        let lr = self.learning_rate;
        let l2 = self.l2_lambda;

        grad_output_b /= batch_size;
        self.output_bias -= lr * grad_output_b;

        for i in 0..self.hidden_size {
            grad_hidden_w[i][0] /= batch_size;
            grad_hidden_b[i] /= batch_size;
            self.hidden_weights[i][0] -= lr * (grad_hidden_w[i][0] + l2 * self.hidden_weights[i][0]);
            self.hidden_bias[i] -= lr * grad_hidden_b[i];

            for j in 0..self.input_size {
                grad_input_w[i][j] /= batch_size;
                self.input_weights[i][j] -= lr * (grad_input_w[i][j] + l2 * self.input_weights[i][j]);
            }
        }

        total_loss / batch_size + self.compute_l2_loss()
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
        self.l2_lambda * 0.5 * loss
    }

    pub fn evaluate(&self, dataset: &[TrainingSample]) -> f64 {
        let mut total_loss = 0.0;
        for sample in dataset {
            let (output, _, _) = self.forward(&sample.inputs, false);
            let error = output - sample.target;
            total_loss += error * error * 0.5;
        }
        total_loss / dataset.len() as f64 + self.compute_l2_loss()
    }

    pub fn train(
        &mut self,
        train_data: &[TrainingSample],
        val_data: &[TrainingSample],
        epochs: usize,
        batch_size: usize,
        patience: usize,
        min_delta: f64,
    ) -> TrainingHistory {
        let mut history = TrainingHistory {
            epochs: 0,
            train_losses: Vec::new(),
            val_losses: Vec::new(),
            best_val_loss: f64::INFINITY,
            best_epoch: 0,
            stopped_early: false,
            patience_used: 0,
        };

        let mut best_weights = (
            self.input_weights.clone(),
            self.hidden_weights.clone(),
            self.hidden_bias.clone(),
            self.output_bias,
        );

        let mut rng = rand::thread_rng();

        for epoch in 0..epochs {
            let mut shuffled_indices: Vec<usize> = (0..train_data.len()).collect();
            for i in (1..shuffled_indices.len()).rev() {
                let j = rng.gen_range(0..=i);
                shuffled_indices.swap(i, j);
            }

            let mut epoch_train_loss = 0.0;
            let mut batch_count = 0;

            for batch_start in (0..train_data.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(train_data.len());
                let batch: Vec<TrainingSample> = shuffled_indices[batch_start..batch_end]
                    .iter()
                    .map(|&i| train_data[i].clone())
                    .collect();

                let loss = self.train_batch(&batch);
                epoch_train_loss += loss;
                batch_count += 1;
            }

            let avg_train_loss = epoch_train_loss / batch_count as f64;
            let val_loss = self.evaluate(val_data);

            history.train_losses.push(avg_train_loss);
            history.val_losses.push(val_loss);
            history.epochs = epoch + 1;

            if epoch == 0 || val_loss < history.best_val_loss - min_delta {
                history.best_val_loss = val_loss;
                history.best_epoch = epoch;
                history.patience_used = 0;
                best_weights = (
                    self.input_weights.clone(),
                    self.hidden_weights.clone(),
                    self.hidden_bias.clone(),
                    self.output_bias,
                );
            } else {
                history.patience_used += 1;
                if history.patience_used >= patience {
                    history.stopped_early = true;
                    self.input_weights = best_weights.0;
                    self.hidden_weights = best_weights.1;
                    self.hidden_bias = best_weights.2;
                    self.output_bias = best_weights.3;
                    break;
                }
            }

            if epoch % 10 == 0 || epoch == epochs - 1 {
                tracing::info!(
                    "Epoch {:4}/{:4} | Train Loss: {:.6} | Val Loss: {:.6} | Best: {:.6} @ {} | Patience: {}/{}",
                    epoch + 1, epochs, avg_train_loss, val_loss,
                    history.best_val_loss, history.best_epoch + 1,
                    history.patience_used, patience
                );
            }
        }

        if !history.stopped_early {
            self.input_weights = best_weights.0;
            self.hidden_weights = best_weights.1;
            self.hidden_bias = best_weights.2;
            self.output_bias = best_weights.3;
        }

        tracing::info!(
            "训练完成: {} epochs, 最佳验证损失 {:.6} @ epoch {}, 早停: {}",
            history.epochs, history.best_val_loss, history.best_epoch + 1, history.stopped_early
        );

        history
    }

    pub fn predict(&self, inputs: &[f64]) -> f64 {
        let (output, _, _) = self.forward(inputs, false);
        output
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let json = fs::read_to_string(path)?;
        let nn: TrainableNeuralNetwork = serde_json::from_str(&json)?;
        Ok(nn)
    }

    pub fn summary(&self) -> String {
        format!(
            "NeuralNetwork(input={}, hidden={}, dropout={}, l2={}, lr={})",
            self.input_size, self.hidden_size,
            self.dropout_rate, self.l2_lambda, self.learning_rate
        )
    }
}

pub fn generate_synthetic_data(count: usize, noise_level: f64) -> Vec<TrainingSample> {
    let mut rng = rand::thread_rng();
    let mut data = Vec::with_capacity(count);

    for _ in 0..count {
        let temp_norm = rng.gen_range(0.0..1.0);
        let hum_norm = rng.gen_range(0.0..1.0);
        let ph_norm = rng.gen_range(0.0..1.0);
        let cl_norm = rng.gen_range(0.0..1.0);
        let material = if rng.gen_bool(0.5) { 1.0 } else { 0.6 };
        let rate_norm = rng.gen_range(0.0..1.0);

        let inputs = vec![temp_norm, hum_norm, ph_norm, cl_norm, material, rate_norm];

        let env_score = temp_norm * 0.15 + hum_norm * 0.3 + ph_norm * 0.15 + cl_norm * 0.3 + 0.1;
        let material_effect = material * 0.2;
        let trend = rate_norm * 0.4;
        let noise = rng.gen_range(-1.0..1.0) * noise_level;
        let target = (env_score + material_effect + trend + noise).max(0.0).min(1.0);

        data.push(TrainingSample { inputs, target });
    }

    data
}

pub fn train_default_model() -> (TrainableNeuralNetwork, TrainingHistory) {
    let all_data = generate_synthetic_data(5000, 0.08);
    let split = (all_data.len() as f64 * 0.8) as usize;
    let (train_data, val_data) = all_data.split_at(split);

    let mut model = TrainableNeuralNetwork::with_hyperparams(6, 16, 0.25, 0.001, 0.01);
    let history = model.train(train_data, val_data, 500, 64, 30, 0.0001);

    (model, history)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_pass() {
        let model = TrainableNeuralNetwork::new(6, 8);
        let inputs = vec![0.5, 0.6, 0.7, 0.3, 1.0, 0.4];
        let output = model.predict(&inputs);
        assert!(output >= 0.0 && output <= 1.5);
    }

    #[test]
    fn test_training() {
        let all_data = generate_synthetic_data(200, 0.05);
        let split = (all_data.len() as f64 * 0.8) as usize;
        let (train_data, val_data) = all_data.split_at(split);

        let mut model = TrainableNeuralNetwork::with_hyperparams(6, 8, 0.1, 0.0001, 0.02);
        let history = model.train(train_data, val_data, 50, 32, 10, 0.001);

        assert!(history.epochs > 0);
        assert!(history.best_val_loss < 0.5);
    }

    #[test]
    fn test_early_stopping() {
        let mut all_data = generate_synthetic_data(300, 0.1);
        for sample in all_data.iter_mut().skip(100) {
            sample.target += 0.5;
        }
        let split = 150;
        let (train_data, val_data) = all_data.split_at(split);

        let mut model = TrainableNeuralNetwork::with_hyperparams(6, 8, 0.2, 0.001, 0.01);
        let history = model.train(train_data, val_data, 200, 32, 15, 0.00001);

        assert!(history.stopped_early || history.epochs == 200);
        assert!(history.best_epoch < history.epochs);
    }

    #[test]
    fn test_save_load() {
        let model = TrainableNeuralNetwork::new(6, 8);
        let inputs = vec![0.3, 0.7, 0.5, 0.2, 1.0, 0.4];
        let original_output = model.predict(&inputs);

        let path = std::env::temp_dir().join("test_nn_model.json");
        model.save(&path).unwrap();
        let loaded = TrainableNeuralNetwork::load(&path).unwrap();
        let loaded_output = loaded.predict(&inputs);

        assert!((original_output - loaded_output).abs() < 1e-6);
        std::fs::remove_file(&path).ok();
    }
}
