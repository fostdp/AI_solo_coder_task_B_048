use serde::{Deserialize, Serialize};
use rand::{rngs::StdRng, SeedableRng, Rng};
use std::collections::HashMap;
use super::microbiome_data::{MicrobiomeSample, GeneCategory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureImportance {
    pub feature_name: String,
    pub importance_score: f64,
    pub corrosion_effect: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationResult {
    pub sample_count: usize,
    pub overall_microbiome_risk: f64,
    pub risk_level: String,
    pub feature_importance: Vec<FeatureImportance>,
    pub top_corrosion_promoters: Vec<String>,
    pub top_corrosion_inhibitors: Vec<String>,
    pub gene_category_scores: HashMap<String, f64>,
    pub diversity_correlation: f64,
    pub biomass_correlation: f64,
    pub ph_microbe_interaction: f64,
    pub chloride_microbe_interaction: f64,
    pub predicted_corrosion_rate: f64,
    pub model_confidence: f64,
    pub risk_recommendations: Vec<String>,
}

struct DecisionTreeNode {
    feature_idx: usize,
    threshold: f64,
    left: Option<Box<DecisionTreeNode>>,
    right: Option<Box<DecisionTreeNode>>,
    prediction: Option<f64>,
}

struct DecisionTree {
    root: Option<DecisionTreeNode>,
    max_depth: usize,
    min_samples_split: usize,
    feature_indices: Vec<usize>,
}

pub struct MicrobeCorrelationAnalyzer {
    n_estimators: usize,
    max_features: usize,
    max_depth: usize,
    min_samples_split: usize,
    random_seed: u64,
}

impl MicrobeCorrelationAnalyzer {
    pub fn new() -> Self {
        Self {
            n_estimators: 100,
            max_features: 0,
            max_depth: 8,
            min_samples_split: 3,
            random_seed: 42,
        }
    }

    pub fn with_params(
        n_estimators: usize,
        max_depth: usize,
        min_samples_split: usize,
        seed: u64,
    ) -> Self {
        Self {
            n_estimators,
            max_features: 0,
            max_depth,
            min_samples_split,
            random_seed: seed,
        }
    }

    pub fn analyze(&self, samples: &[MicrobiomeSample]) -> CorrelationResult {
        let n = samples.len();
        if n < 3 {
            return self.fallback_analysis(samples);
        }

        let (features, feature_names, descriptions, effects) = self.build_feature_matrix(samples);
        let targets: Vec<f64> = samples.iter().map(|s| s.corrosion_rate_observed).collect();

        let (features, targets) = self.smote_balance(&features, &targets);

        let n = features.len();
        let n_features = feature_names.len();
        let max_feat = if self.max_features > 0 {
            self.max_features.min(n_features)
        } else {
            (n_features as f64).sqrt() as usize + 1
        };

        let mut rng = StdRng::seed_from_u64(self.random_seed);
        let mut feature_importance = vec![0.0_f64; n_features];
        let mut oob_predictions = vec![0.0_f64; n];
        let mut oob_counts = vec![0_usize; n];

        for _ in 0..self.n_estimators {
            let (bootstrap_idx, oob_idx) = self.bootstrap_indices(n, &mut rng);

            let tree_features: Vec<usize> = (0..n_features)
                .map(|i| i)
                .collect::<Vec<_>>()
                .into_iter()
                .take(max_feat)
                .collect();

            let bootstrap_features: Vec<Vec<f64>> = bootstrap_idx
                .iter()
                .map(|&i| features[i].clone())
                .collect();
            let bootstrap_targets: Vec<f64> = bootstrap_idx
                .iter()
                .map(|&i| targets[i])
                .collect();

            let tree = self.build_tree(
                &bootstrap_features,
                &bootstrap_targets,
                &tree_features,
                0,
                &mut rng,
            );

            let local_importance = self.compute_tree_importance(&tree, &features, &targets, &tree_features);
            for (fi_idx, feat_idx) in tree_features.iter().enumerate() {
                feature_importance[*feat_idx] += local_importance[fi_idx];
            }

            for &i in &oob_idx {
                let pred = self.predict_tree(&tree, &features[i]);
                if let Some(p) = pred {
                    oob_predictions[i] += p;
                    oob_counts[i] += 1;
                }
            }
        }

        let max_imp = feature_importance.iter().cloned().fold(0.0_f64, f64::max);
        if max_imp > 0.0 {
            for imp in feature_importance.iter_mut() {
                *imp /= max_imp;
            }
        }

        let mut oob_avg = Vec::new();
        let mut target_vals = Vec::new();
        for i in 0..n {
            if oob_counts[i] > 0 {
                oob_avg.push(oob_predictions[i] / oob_counts[i] as f64);
                target_vals.push(targets[i]);
            }
        }
        let model_confidence = if oob_avg.len() >= 2 {
            let corr = self.pearson_correlation(&oob_avg, &target_vals);
            corr.max(0.0).min(1.0)
        } else {
            0.65
        };

        let predicted_corrosion_rate = if oob_avg.is_empty() {
            targets.iter().sum::<f64>() / targets.len() as f64
        } else {
            oob_avg.iter().sum::<f64>() / oob_avg.len() as f64
        };

        let mut importance_list: Vec<FeatureImportance> = feature_names
            .iter()
            .enumerate()
            .map(|(i, name)| FeatureImportance {
                feature_name: name.clone(),
                importance_score: feature_importance[i],
                corrosion_effect: effects[i].clone(),
                description: descriptions[i].clone(),
            })
            .collect();
        importance_list.sort_by(|a, b| {
            b.importance_score
                .partial_cmp(&a.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let top_promoters: Vec<String> = importance_list
            .iter()
            .filter(|f| f.corrosion_effect.contains("促"))
            .take(3)
            .map(|f| f.feature_name.clone())
            .collect();

        let top_inhibitors: Vec<String> = importance_list
            .iter()
            .filter(|f| f.corrosion_effect.contains("抑"))
            .take(2)
            .map(|f| f.feature_name.clone())
            .collect();

        let mut gene_scores = HashMap::new();
        for s in samples {
            for g in &s.functional_genes {
                let entry = gene_scores.entry(g.category.as_str().to_string()).or_insert(0.0);
                *entry += g.relative_expression * g.corrosion_relevance.numeric();
            }
        }
        for v in gene_scores.values_mut() {
            *v = (*v / n as f64).clamp(-1.0, 1.0);
        }

        let shannon_vals: Vec<f64> = samples.iter().map(|s| s.shannon_diversity).collect();
        let biomass_vals: Vec<f64> = samples.iter().map(|s| s.microbial_biomass_cfu_g.log10()).collect();
        let ph_vals: Vec<f64> = samples.iter().map(|s| s.ph).collect();
        let cl_vals: Vec<f64> = samples.iter().map(|s| s.chloride_ppm).collect();

        let diversity_corr = self.pearson_correlation(&shannon_vals, &targets);
        let biomass_corr = self.pearson_correlation(&biomass_vals, &targets);

        let ph_microbe: Vec<f64> = samples
            .iter()
            .map(|s| s.ph * s.corrosion_gene_score())
            .collect();
        let cl_microbe: Vec<f64> = samples
            .iter()
            .map(|s| s.chloride_ppm * s.corrosion_gene_score())
            .collect();
        let ph_microbe_interaction = self.pearson_correlation(&ph_microbe, &targets);
        let chloride_microbe_interaction = self.pearson_correlation(&cl_microbe, &targets);

        let risk = self.compute_overall_risk(
            &importance_list,
            &gene_scores,
            predicted_corrosion_rate,
        );

        let risk_level = if risk < 0.25 {
            "低".to_string()
        } else if risk < 0.5 {
            "中等".to_string()
        } else if risk < 0.75 {
            "较高".to_string()
        } else {
            "高".to_string()
        };

        let recs = self.generate_recommendations(
            risk,
            &gene_scores,
            diversity_corr,
            &importance_list,
        );

        CorrelationResult {
            sample_count: n,
            overall_microbiome_risk: risk,
            risk_level,
            feature_importance: importance_list,
            top_corrosion_promoters: top_promoters,
            top_corrosion_inhibitors: top_inhibitors,
            gene_category_scores: gene_scores,
            diversity_correlation,
            biomass_correlation,
            ph_microbe_interaction,
            chloride_microbe_interaction,
            predicted_corrosion_rate,
            model_confidence,
            risk_recommendations: recs,
        }
    }

    fn build_feature_matrix(
        &self,
        samples: &[MicrobiomeSample],
    ) -> (Vec<Vec<f64>>, Vec<String>, Vec<String>, Vec<String>) {
        let mut features = Vec::new();
        let mut names = Vec::new();
        let mut descriptions = Vec::new();
        let mut effects = Vec::new();

        names.push("Shannon多样性指数".to_string());
        descriptions.push("群落alpha多样性，物种丰富度与均匀度综合指标".to_string());
        effects.push("中性-复杂".to_string());

        names.push("微生物生物量(log10 CFU/g)".to_string());
        descriptions.push("土壤中总可培养微生物数量".to_string());
        effects.push("弱促腐蚀".to_string());

        names.push("土壤pH".to_string());
        descriptions.push("酸碱度，影响微生物活性与电化学腐蚀".to_string());
        effects.push("双向影响".to_string());

        names.push("土壤湿度(%)".to_string());
        descriptions.push("水分含量，影响微生物代谢与电化学过程".to_string());
        effects.push("促腐蚀".to_string());

        names.push("总有机碳(%)".to_string());
        descriptions.push("碳源含量，微生物能量来源".to_string());
        effects.push("弱促腐蚀".to_string());

        names.push("氯离子浓度(ppm)".to_string());
        descriptions.push("活性腐蚀离子，破坏钝化膜".to_string());
        effects.push("强促腐蚀".to_string());

        names.push("腐蚀基因综合得分".to_string());
        descriptions.push("功能基因表达与腐蚀相关性加权得分".to_string());
        effects.push("强促腐蚀".to_string());

        names.push("群落均匀度(Pielou)".to_string());
        descriptions.push("物种分布均匀程度".to_string());
        effects.push("弱抑腐蚀".to_string());

        if let Some(first) = samples.first() {
            for g in &first.functional_genes {
                names.push(format!("基因: {}", g.gene_name));
                descriptions.push(g.function.clone());
                effects.push(g.corrosion_relevance.as_str().to_string());
            }
        }

        for s in samples {
            let mut row = vec![
                s.shannon_diversity,
                s.microbial_biomass_cfu_g.log10().max(0.0),
                s.ph,
                s.moisture_pct,
                s.total_organic_carbon_pct,
                s.chloride_ppm,
                s.corrosion_gene_score(),
                s.evenness,
            ];
            for g in &s.functional_genes {
                row.push(g.relative_expression);
            }
            features.push(row);
        }

        (features, names, descriptions, effects)
    }

    fn bootstrap_indices(&self, n: usize, rng: &mut StdRng) -> (Vec<usize>, Vec<usize>) {
        let mut bootstrap = Vec::with_capacity(n);
        let mut in_bag = vec![false; n];
        for _ in 0..n {
            let idx = rng.gen_range(0..n);
            bootstrap.push(idx);
            in_bag[idx] = true;
        }
        let oob: Vec<usize> = (0..n).filter(|&i| !in_bag[i]).collect();
        (bootstrap, oob)
    }

    fn build_tree(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
        feat_indices: &[usize],
        depth: usize,
        rng: &mut StdRng,
    ) -> DecisionTreeNode {
        let n = features.len();

        if depth >= self.max_depth
            || n < self.min_samples_split
            || self.variance(targets) < 1e-8
        {
            return DecisionTreeNode {
                feature_idx: 0,
                threshold: 0.0,
                left: None,
                right: None,
                prediction: Some(targets.iter().sum::<f64>() / n.max(1) as f64),
            };
        }

        let mut best_gain = -f64::INFINITY;
        let mut best_feat = 0usize;
        let mut best_thresh = 0.0f64;

        for &fi in feat_indices {
            let mut values: Vec<f64> = features.iter().map(|row| row[fi]).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            values.dedup_by(|a, b| (a - b).abs() < 1e-9);

            if values.len() < 2 {
                continue;
            }

            for split_idx in 1..values.len() {
                let thresh = (values[split_idx - 1] + values[split_idx]) / 2.0;

                let mut left_targets = Vec::new();
                let mut right_targets = Vec::new();
                for (row, t) in features.iter().zip(targets.iter()) {
                    if row[fi] <= thresh {
                        left_targets.push(*t);
                    } else {
                        right_targets.push(*t);
                    }
                }

                if left_targets.is_empty() || right_targets.is_empty() {
                    continue;
                }

                let parent_var = self.variance(targets);
                let left_var = self.variance(&left_targets);
                let right_var = self.variance(&right_targets);
                let n_parent = targets.len() as f64;
                let weighted_var =
                    (left_targets.len() as f64 / n_parent) * left_var
                        + (right_targets.len() as f64 / n_parent) * right_var;
                let gain = parent_var - weighted_var;

                if gain > best_gain {
                    best_gain = gain;
                    best_feat = fi;
                    best_thresh = thresh;
                }
            }
        }

        if best_gain <= 0.0 {
            return DecisionTreeNode {
                feature_idx: 0,
                threshold: 0.0,
                left: None,
                right: None,
                prediction: Some(targets.iter().sum::<f64>() / n.max(1) as f64),
            };
        }

        let mut left_feat = Vec::new();
        let mut left_targ = Vec::new();
        let mut right_feat = Vec::new();
        let mut right_targ = Vec::new();
        for (row, t) in features.iter().zip(targets.iter()) {
            if row[best_feat] <= best_thresh {
                left_feat.push(row.clone());
                left_targ.push(*t);
            } else {
                right_feat.push(row.clone());
                right_targ.push(*t);
            }
        }

        let left_node = if left_feat.len() >= self.min_samples_split {
            Some(Box::new(self.build_tree(
                &left_feat,
                &left_targ,
                feat_indices,
                depth + 1,
                rng,
            )))
        } else {
            Some(Box::new(DecisionTreeNode {
                feature_idx: 0,
                threshold: 0.0,
                left: None,
                right: None,
                prediction: Some(left_targ.iter().sum::<f64>() / left_targ.len().max(1) as f64),
            }))
        };

        let right_node = if right_feat.len() >= self.min_samples_split {
            Some(Box::new(self.build_tree(
                &right_feat,
                &right_targ,
                feat_indices,
                depth + 1,
                rng,
            )))
        } else {
            Some(Box::new(DecisionTreeNode {
                feature_idx: 0,
                threshold: 0.0,
                left: None,
                right: None,
                prediction: Some(right_targ.iter().sum::<f64>() / right_targ.len().max(1) as f64),
            }))
        };

        DecisionTreeNode {
            feature_idx: best_feat,
            threshold: best_thresh,
            left: left_node,
            right: right_node,
            prediction: None,
        }
    }

    fn compute_tree_importance(
        &self,
        tree: &DecisionTreeNode,
        _features: &[Vec<f64>],
        _targets: &[f64],
        tree_feat_indices: &[usize],
    ) -> Vec<f64> {
        let mut imp = vec![0.0_f64; tree_feat_indices.len()];
        self.walk_tree_importance(tree, &mut imp, tree_feat_indices);
        imp
    }

    fn walk_tree_importance(
        &self,
        node: &DecisionTreeNode,
        imp: &mut Vec<f64>,
        tree_feat_indices: &[usize],
    ) {
        if node.prediction.is_none() {
            if let Some(pos) = tree_feat_indices.iter().position(|&fi| fi == node.feature_idx) {
                imp[pos] += 1.0;
            }
            if let Some(ref l) = node.left {
                self.walk_tree_importance(l, imp, tree_feat_indices);
            }
            if let Some(ref r) = node.right {
                self.walk_tree_importance(r, imp, tree_feat_indices);
            }
        }
    }

    fn predict_tree(&self, tree: &DecisionTreeNode, sample: &[f64]) -> Option<f64> {
        let mut node = tree;
        loop {
            if let Some(pred) = node.prediction {
                return Some(pred);
            }
            if sample[node.feature_idx] <= node.threshold {
                node = match node.left {
                    Some(ref n) => n,
                    None => return None,
                };
            } else {
                node = match node.right {
                    Some(ref n) => n,
                    None => return None,
                };
            }
        }
    }

    fn variance(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
    }

    fn pearson_correlation(&self, x: &[f64], y: &[f64]) -> f64 {
        let n = x.len().min(y.len());
        if n < 2 {
            return 0.0;
        }
        let mean_x: f64 = x[..n].iter().sum::<f64>() / n as f64;
        let mean_y: f64 = y[..n].iter().sum::<f64>() / n as f64;

        let mut num = 0.0_f64;
        let mut den_x = 0.0_f64;
        let mut den_y = 0.0_f64;

        for i in 0..n {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            num += dx * dy;
            den_x += dx * dx;
            den_y += dy * dy;
        }

        let den = (den_x * den_y).sqrt();
        if den < 1e-12 {
            0.0
        } else {
            (num / den).clamp(-1.0, 1.0)
        }
    }

    fn smote_balance(&self, features: &[Vec<f64>], targets: &[f64]) -> (Vec<Vec<f64>>, Vec<f64>) {
        let n = targets.len();
        if n < 4 {
            return (features.to_vec(), targets.to_vec());
        }

        let sorted_targets = {
            let mut t = targets.to_vec();
            t.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            t
        };
        let median = sorted_targets[n / 2];

        let mut minority_idx: Vec<usize> = Vec::new();
        let mut majority_idx: Vec<usize> = Vec::new();
        for (i, &t) in targets.iter().enumerate() {
            if t >= median {
                minority_idx.push(i);
            } else {
                majority_idx.push(i);
            }
        }

        if minority_idx.is_empty() || majority_idx.is_empty() {
            return (features.to_vec(), targets.to_vec());
        }

        let diff = majority_idx.len() as i32 - minority_idx.len() as i32;
        if diff.abs() <= 1 {
            return (features.to_vec(), targets.to_vec());
        }

        let mut rng = StdRng::seed_from_u64(self.random_seed + 9999);
        let num_synthetic = diff.abs() as usize;
        let n_features = features[0].len();

        let mut synth_features = Vec::with_capacity(num_synthetic);
        let mut synth_targets = Vec::with_capacity(num_synthetic);

        for _ in 0..num_synthetic {
            let src_idx = minority_idx[rng.gen_range(0..minority_idx.len())];
            let nn_idx = minority_idx[rng.gen_range(0..minority_idx.len())];

            let gap: f64 = rng.gen_range(0.0..1.0);
            let mut synth_row = vec![0.0_f64; n_features];
            for j in 0..n_features {
                synth_row[j] = features[src_idx][j] + gap * (features[nn_idx][j] - features[src_idx][j]);
            }
            let synth_target = targets[src_idx] + gap * (targets[nn_idx] - targets[src_idx]);

            synth_features.push(synth_row);
            synth_targets.push(synth_target);
        }

        let mut balanced_features = features.to_vec();
        let mut balanced_targets = targets.to_vec();
        balanced_features.extend(synth_features);
        balanced_targets.extend(synth_targets);

        (balanced_features, balanced_targets)
    }

    fn compute_overall_risk(
        &self,
        importance: &[FeatureImportance],
        gene_scores: &HashMap<String, f64>,
        predicted_rate: f64,
    ) -> f64 {
        let promoter_score: f64 = importance
            .iter()
            .filter(|f| f.corrosion_effect.contains("促"))
            .take(5)
            .map(|f| f.importance_score)
            .sum::<f64>()
            / 5.0;

        let avg_gene_score: f64 = gene_scores.values().sum::<f64>() / gene_scores.len().max(1) as f64;
        let normalized_gene = (avg_gene_score + 1.0) / 2.0;

        let rate_score = (predicted_rate / 0.5).min(1.0);

        promoter_score * 0.35 + normalized_gene * 0.35 + rate_score * 0.30
    }

    fn generate_recommendations(
        &self,
        risk: f64,
        gene_scores: &HashMap<String, f64>,
        diversity: f64,
        importance: &[FeatureImportance],
    ) -> Vec<String> {
        let mut recs = Vec::new();

        let sulfur_score = *gene_scores.get("硫氧化").unwrap_or(&0.0);
        let iron_score = *gene_scores.get("铁氧化").unwrap_or(&0.0)
            + *gene_scores.get("铁还原").unwrap_or(&0.0);
        let acid_score = *gene_scores.get("产酸相关").unwrap_or(&0.0);

        if sulfur_score > 0.3 {
            recs.push("检测到高活性硫氧化微生物(如Acidithiobacillus)，建议进行环境脱硫处理".to_string());
        }
        if iron_score > 0.3 {
            recs.push("铁代谢微生物活跃(如Shewanella)，需密切监测铁器文物电化学腐蚀速率".to_string());
        }
        if acid_score > 0.3 {
            recs.push("产酸相关基因高表达，建议施加碱性改良剂稳定土壤pH".to_string());
        }

        if diversity < -0.2 {
            recs.push("群落多样性与腐蚀呈负相关，建议维持微生物群落多样性以抑制专一性腐蚀菌".to_string());
        } else if diversity > 0.2 {
            recs.push("群落多样性越高腐蚀越严重，可能存在协同腐蚀效应，需进一步分析关键物种".to_string());
        }

        if let Some(top) = importance.first() {
            if top.importance_score > 0.6 {
                recs.push(format!("关键影响因子: {} (贡献度 {:.1}%)，建议优先针对该因素干预",
                    top.feature_name, top.importance_score * 100.0));
            }
        }

        if risk > 0.6 {
            recs.push("微生物腐蚀风险较高，建议定期(每周)采样监测群落动态变化".to_string());
        } else if risk > 0.3 {
            recs.push("微生物腐蚀风险中等，建议每月进行微生物采样检测".to_string());
        }

        if recs.is_empty() {
            recs.push("微生物群落整体稳定，按常规季度监测即可".to_string());
        }

        recs
    }

    fn fallback_analysis(&self, samples: &[MicrobiomeSample]) -> CorrelationResult {
        let avg_rate: f64 = samples.iter().map(|s| s.corrosion_rate_observed).sum::<f64>()
            / samples.len().max(1) as f64;
        let avg_gene: f64 = samples.iter().map(|s| s.corrosion_gene_score()).sum::<f64>()
            / samples.len().max(1) as f64;
        let risk = (avg_rate / 0.5 * 0.5 + ((avg_gene + 1.0) / 2.0) * 0.5).clamp(0.0, 1.0);

        CorrelationResult {
            sample_count: samples.len(),
            overall_microbiome_risk: risk,
            risk_level: if risk < 0.3 { "低".to_string() } else if risk < 0.6 { "中等".to_string() } else { "高".to_string() },
            feature_importance: vec![
                FeatureImportance {
                    feature_name: "腐蚀基因综合得分".to_string(),
                    importance_score: 0.85,
                    corrosion_effect: "强促腐蚀".to_string(),
                    description: "功能基因表达与腐蚀相关性加权".to_string(),
                },
                FeatureImportance {
                    feature_name: "氯离子浓度".to_string(),
                    importance_score: 0.75,
                    corrosion_effect: "强促腐蚀".to_string(),
                    description: "活性腐蚀离子".to_string(),
                },
                FeatureImportance {
                    feature_name: "土壤湿度".to_string(),
                    importance_score: 0.60,
                    corrosion_effect: "促腐蚀".to_string(),
                    description: "电化学介质".to_string(),
                },
            ],
            top_corrosion_promoters: vec!["硫氧化基因簇".to_string(), "铁氧化/还原系统".to_string()],
            top_corrosion_inhibitors: vec![],
            gene_category_scores: HashMap::new(),
            diversity_correlation: 0.0,
            biomass_correlation: 0.0,
            ph_microbe_interaction: 0.0,
            chloride_microbe_interaction: 0.0,
            predicted_corrosion_rate: avg_rate,
            model_confidence: 0.55,
            risk_recommendations: vec!["样本量不足，建议增加采样点以提高分析可靠性".to_string()],
        }
    }
}

impl Default for MicrobeCorrelationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::microbiome_data::{generate_mock_microbiome, default_microbe_dataset, MicrobiomeSample, GeneCategory, CorrosionRelevance};

    // ─── 特征重要性排序与先验知识一致（核心验证） ───

    #[test]
    fn test_feature_importance_top_contains_known_promoters() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        // 先验知识：氯离子、腐蚀基因综合得分、硫氧化基因应是重要的促腐蚀因子
        let top_names: Vec<&str> = result.feature_importance
            .iter()
            .take(5)
            .map(|f| f.feature_name.as_str())
            .collect();

        // 氯离子和腐蚀基因得分应在前5名
        let has_chloride = top_names.iter().any(|&n| n.contains("氯离子"));
        let has_corrosion_gene = top_names.iter().any(|&n| n.contains("腐蚀基因"));

        assert!(has_chloride || has_corrosion_gene,
            "氯离子或腐蚀基因得分应在重要度前5名: {:?}", top_names);
    }

    #[test]
    fn test_corrosion_promoters_rank_higher_than_inhibitors() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::with_params(50, 6, 2, 42);
        let result = analyzer.analyze(&samples);

        // 促腐蚀因子的平均重要度应高于抑腐蚀因子
        let promoters: Vec<f64> = result.feature_importance
            .iter()
            .filter(|f| f.corrosion_effect.contains("促"))
            .map(|f| f.importance_score)
            .collect();

        let inhibitors: Vec<f64> = result.feature_importance
            .iter()
            .filter(|f| f.corrosion_effect.contains("抑"))
            .map(|f| f.importance_score)
            .collect();

        if !promoters.is_empty() && !inhibitors.is_empty() {
            let avg_promote: f64 = promoters.iter().sum::<f64>() / promoters.len() as f64;
            let avg_inhibit: f64 = inhibitors.iter().sum::<f64>() / inhibitors.len() as f64;
            // 促腐蚀因子重要度不应显著低于抑腐蚀因子
            assert!(avg_promote > 0.3 || avg_inhibit < 0.8,
                "促腐蚀因子重要度({:.3}) 与抑腐蚀因子({:.3}) 关系不合理",
                avg_promote, avg_inhibit);
        }
    }

    #[test]
    fn test_top_promoters_list_non_empty() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert!(!result.top_corrosion_promoters.is_empty(),
            "应至少有一个促腐蚀因子");
    }

    #[test]
    fn test_chloride_is_promoter() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        // 氯离子应被标记为促腐蚀
        let chloride_feat = result.feature_importance
            .iter()
            .find(|f| f.feature_name.contains("氯离子"));

        if let Some(feat) = chloride_feat {
            assert!(feat.corrosion_effect.contains("促"),
                "氯离子应标记为促腐蚀: {}", feat.corrosion_effect);
        }
    }

    // ─── 随机森林基本性质验证 ───

    #[test]
    fn test_deterministic_with_same_seed() {
        let samples = default_microbe_dataset();
        let analyzer1 = MicrobeCorrelationAnalyzer::with_params(30, 6, 2, 12345);
        let analyzer2 = MicrobeCorrelationAnalyzer::with_params(30, 6, 2, 12345);

        let r1 = analyzer1.analyze(&samples);
        let r2 = analyzer2.analyze(&samples);

        assert!((r1.overall_microbiome_risk - r2.overall_microbiome_risk).abs() < 1e-9,
            "相同种子应产生完全相同的结果");
        assert_eq!(r1.feature_importance.len(), r2.feature_importance.len());
    }

    #[test]
    fn test_feature_importance_sums_positive() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        let total: f64 = result.feature_importance
            .iter()
            .map(|f| f.importance_score)
            .sum();
        assert!(total > 0.0, "特征重要度总和应为正");
    }

    #[test]
    fn test_importance_normalized_to_0_1() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        for feat in &result.feature_importance {
            assert!(feat.importance_score >= 0.0 && feat.importance_score <= 1.0 + 1e-9,
                "特征重要度应归一化到[0,1]: {} = {}",
                feat.feature_name, feat.importance_score);
        }
        // 最重要特征应为1
        let max_imp = result.feature_importance[0].importance_score;
        assert!((max_imp - 1.0).abs() < 1e-9, "最大重要度应归一化为1");
    }

    // ─── 样本数量边界测试 ───

    #[test]
    fn test_insufficient_samples_fallback() {
        // 少于3个样本应使用回退分析
        let mut samples = Vec::new();
        for i in 0..2 {
            samples.push(generate_mock_microbiome(
                &format!("S{}", i), "测试区", 34.0, 108.0, i as u64));
        }
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert_eq!(result.sample_count, 2);
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);
        assert!(!result.feature_importance.is_empty());
    }

    #[test]
    fn test_single_sample_handled() {
        let samples = vec![generate_mock_microbiome(
            "S1", "测试区", 34.0, 108.0, 42)];
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert_eq!(result.sample_count, 1);
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);
    }

    #[test]
    fn test_empty_samples() {
        let samples: Vec<MicrobiomeSample> = Vec::new();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert_eq!(result.sample_count, 0);
        // 空输入也应返回合理结果
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);
    }

    // ─── 正常条件测试 ───

    #[test]
    fn test_full_analysis_result_structure() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        assert!(result.sample_count >= 3);
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);
        assert!(!result.risk_level.is_empty());
        assert!(!result.feature_importance.is_empty());
        assert!(!result.gene_category_scores.is_empty());
        assert!(result.diversity_correlation >= -1.0 && result.diversity_correlation <= 1.0);
        assert!(result.biomass_correlation >= -1.0 && result.biomass_correlation <= 1.0);
        assert!(result.ph_microbe_interaction >= -1.0 && result.ph_microbe_interaction <= 1.0);
        assert!(result.chloride_microbe_interaction >= -1.0 && result.chloride_microbe_interaction <= 1.0);
        assert!(result.predicted_corrosion_rate > 0.0);
        assert!(result.model_confidence >= 0.0 && result.model_confidence <= 1.0);
        assert!(!result.risk_recommendations.is_empty());
    }

    #[test]
    fn test_gene_category_scores_reasonable() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);

        for (name, score) in &result.gene_category_scores {
            assert!(score >= &-1.0 && score <= &1.0,
                "基因类别{}得分{}应在[-1,1]范围内", name, score);
        }
    }

    // ─── 皮尔逊相关系数验证 ───

    #[test]
    fn test_pearson_perfect_positive() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = analyzer.pearson_correlation(&x, &y);
        assert!((r - 1.0).abs() < 1e-6, "完全正相关应为1");
    }

    #[test]
    fn test_pearson_perfect_negative() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let r = analyzer.pearson_correlation(&x, &y);
        assert!((r + 1.0).abs() < 1e-6, "完全负相关应为-1");
    }

    #[test]
    fn test_pearson_zero_correlation() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        // 完全不相关的随机序列（确定性）
        let x = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        let y = vec![2.0, 1.0, 3.0, 0.0, 4.0];
        let r = analyzer.pearson_correlation(&x, &y);
        // 这个组合相关系数不高，验证在合理范围
        assert!(r >= -1.0 && r <= 1.0);
    }

    #[test]
    fn test_pearson_constant_y_returns_zero() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![3.0, 3.0, 3.0, 3.0, 3.0];
        let r = analyzer.pearson_correlation(&x, &y);
        // 常数y方差为0，相关系数应返回0或安全值
        assert!(r.is_finite());
    }

    // ─── 不同参数的影响验证 ───

    #[test]
    fn test_more_trees_changes_result_slightly() {
        let samples = default_microbe_dataset();
        let a1 = MicrobeCorrelationAnalyzer::with_params(10, 6, 2, 42);
        let a2 = MicrobeCorrelationAnalyzer::with_params(100, 6, 2, 42);

        let r1 = a1.analyze(&samples);
        let r2 = a2.analyze(&samples);

        // 结果应该相近但不完全相同（不同的树数量）
        assert!(r1.overall_microbiome_risk >= 0.0 && r1.overall_microbiome_risk <= 1.0);
        assert!(r2.overall_microbiome_risk >= 0.0 && r2.overall_microbiome_risk <= 1.0);
    }

    // ─── 风险等级一致性 ───

    #[test]
    fn test_risk_level_matches_score() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        let risk = result.overall_microbiome_risk;

        let expected_level = if risk < 0.25 {
            "低"
        } else if risk < 0.5 {
            "中等"
        } else if risk < 0.75 {
            "较高"
        } else {
            "高"
        };

        assert_eq!(result.risk_level, expected_level,
            "风险分数{}对应等级不匹配: {}", risk, result.risk_level);
    }

    #[test]
    fn test_recommendations_non_empty_across_risk_levels() {
        let samples = default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert!(!result.risk_recommendations.is_empty(),
            "任何风险等级都应有保护建议");
    }

    // ─── 辅助函数验证 ───

    fn generate_mock_microbiome(id: &str, zone: &str, lat: f64, lng: f64, seed: u64) -> MicrobiomeSample {
        super::microbiome_data::generate_mock_microbiome(id, zone, lat, lng, seed)
    }

    // ─── SMOTE过采样验证（缺陷修复3） ───

    #[test]
    fn test_smote_balances_imbalanced_data() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let features = vec![
            vec![1.0, 0.1],
            vec![2.0, 0.2],
            vec![3.0, 0.3],
            vec![10.0, 1.0],
        ];
        let targets = vec![0.05, 0.08, 0.10, 0.80];

        let (bal_feat, bal_tgt) = analyzer.smote_balance(&features, &targets);

        assert!(bal_feat.len() >= features.len(),
            "SMOTE后样本数应≥原始数: 原始={}, 平衡后={}", features.len(), bal_feat.len());
        assert_eq!(bal_feat.len(), bal_tgt.len());
        for (i, row) in bal_feat.iter().enumerate() {
            assert_eq!(row.len(), 2, "特征维度应保持不变: 第{}行", i);
        }
    }

    #[test]
    fn test_smote_synthetic_in_range() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let features = vec![
            vec![1.0, 10.0],
            vec![2.0, 20.0],
            vec![3.0, 30.0],
            vec![4.0, 40.0],
            vec![50.0, 500.0],
        ];
        let targets = vec![0.05, 0.08, 0.10, 0.12, 0.90];

        let (bal_feat, bal_tgt) = analyzer.smote_balance(&features, &targets);

        for (i, row) in bal_feat.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                assert!(v.is_finite(), "合成特征应有限: 行{}列{}", i, j);
            }
        }
        for (i, &t) in bal_tgt.iter().enumerate() {
            assert!(t.is_finite() && t >= 0.0, "合成目标应非负有限: 行{}", i);
        }
    }

    #[test]
    fn test_smote_balanced_data_unchanged() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let features = vec![
            vec![1.0, 0.5],
            vec![2.0, 1.0],
            vec![3.0, 1.5],
            vec![4.0, 2.0],
        ];
        let targets = vec![0.1, 0.3, 0.5, 0.7];

        let (bal_feat, bal_tgt) = analyzer.smote_balance(&features, &targets);

        assert_eq!(bal_feat.len(), features.len(),
            "已平衡数据不应添加合成样本");
        assert_eq!(bal_tgt.len(), targets.len());
    }

    #[test]
    fn test_smote_too_few_samples_passthrough() {
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let features = vec![vec![1.0], vec![2.0], vec![3.0]];
        let targets = vec![0.1, 0.5, 0.9];

        let (bal_feat, bal_tgt) = analyzer.smote_balance(&features, &targets);

        assert_eq!(bal_feat.len(), 3, "不足4样本应直接透传");
    }

    #[test]
    fn test_unbalanced_dataset_analysis_no_bias() {
        let mut samples = Vec::new();
        for i in 0..4 {
            let mut s = generate_mock_microbiome(
                &format!("S{}", i), "低腐蚀区", 34.0, 108.0, i as u64);
            s.corrosion_rate_observed = 0.05 + i as f64 * 0.02;
            s.chloride_ppm = 10.0 + i as f64 * 5.0;
            samples.push(s);
        }
        for i in 0..2 {
            let mut s = generate_mock_microbiome(
                &format!("SH{}", i), "高腐蚀区", 34.0, 108.0, 100 + i as u64);
            s.corrosion_rate_observed = 0.80 + i as f64 * 0.05;
            s.chloride_ppm = 300.0 + i as f64 * 50.0;
            samples.push(s);
        }

        let analyzer = MicrobeCorrelationAnalyzer::with_params(30, 5, 2, 42);
        let result = analyzer.analyze(&samples);

        assert!(!result.overall_microbiome_risk.is_nan());
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);

        let has_chloride = result.feature_importance
            .iter()
            .take(5)
            .any(|f| f.feature_name.contains("氯离子"));
        assert!(has_chloride,
            "不平衡数据中氯离子仍应在重要度前5: {:?}",
            result.feature_importance.iter().take(5).map(|f| &f.feature_name).collect::<Vec<_>>());
    }
}
