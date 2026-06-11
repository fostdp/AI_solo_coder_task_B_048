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

    #[test]
    fn test_analyze_with_mock_data() {
        let samples = super::microbiome_data::default_microbe_dataset();
        let analyzer = MicrobeCorrelationAnalyzer::new();
        let result = analyzer.analyze(&samples);
        assert!(result.sample_count > 0);
        assert!(result.overall_microbiome_risk >= 0.0 && result.overall_microbiome_risk <= 1.0);
    }
}
