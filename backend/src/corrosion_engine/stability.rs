use crate::common::models::StabilityAssessment;

pub struct StabilityAnalyzer;

impl StabilityAnalyzer {
    pub fn assess(
        probe_id: &str,
        material_type: &str,
        corrosion_rate: f64,
        temp: f64,
        hum: f64,
        ph: f64,
        chloride: f64,
    ) -> StabilityAssessment {
        let env_score = Self::env_calc(temp, hum, ph, chloride);
        let corrosion_factor = (corrosion_rate / 1.0).min(1.0);
        let stability_index = 1.0 - env_score * 0.6 - corrosion_factor * 0.4;

        let stability_level = if stability_index > 0.85 {
            "极稳定".to_string()
        } else if stability_index > 0.7 {
            "稳定".to_string()
        } else if stability_index > 0.5 {
            "较稳定".to_string()
        } else if stability_index > 0.3 {
            "不稳定".to_string()
        } else {
            "极不稳定".to_string()
        };

        let is_iron = material_type == "iron"
            || material_type == "Iron"
            || material_type == "铁";

        let remaining_lifetime_years = if is_iron {
            stability_index * stability_index * stability_index * 500.0
        } else {
            stability_index.powf(1.5) * 1000.0
        };

        let mut recommendations = Vec::new();

        let temp_score = ((temp - 5.0) / 35.0).clamp(0.0, 1.0);
        let hum_score = (hum / 100.0).clamp(0.0, 1.0);
        let ph_score = ((ph - 4.0) / 10.0).clamp(0.0, 1.0);
        let cl_score = (chloride / 200.0).clamp(0.0, 1.0);

        if temp_score > 0.7 {
            recommendations.push("环境温度过高，建议采取降温措施".to_string());
        }

        if hum_score > 0.8 {
            recommendations.push("湿度过高，建议加强通风除湿".to_string());
        }

        if ph_score < 0.2 {
            recommendations.push("土壤酸性过强，建议施加碱性改良剂".to_string());
        } else if ph_score > 0.8 {
            recommendations.push("土壤碱性过强，建议施加酸性改良剂".to_string());
        }

        if cl_score > 0.6 {
            recommendations.push("氯离子含量偏高，建议进行脱盐处理".to_string());
        }

        if corrosion_rate > 0.3 {
            recommendations.push("腐蚀速率较高，建议立即涂刷防腐涂层".to_string());
        } else if corrosion_rate > 0.1 {
            recommendations.push("腐蚀速率中等，建议定期检查防腐层完整性".to_string());
        }

        if stability_index < 0.3 {
            recommendations.push("稳定性极差，建议增加监测频率至每小时一次".to_string());
        } else if stability_index < 0.5 {
            recommendations.push("稳定性较差，建议增加监测频率至每日四次".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("环境条件稳定，继续按常规频率监测".to_string());
        }

        StabilityAssessment {
            probe_id: probe_id.to_string(),
            material_type: material_type.to_string(),
            stability_index,
            stability_level,
            env_score,
            corrosion_factor,
            remaining_lifetime_years,
            recommendations,
        }
    }

    fn env_calc(temp: f64, hum: f64, ph: f64, chloride: f64) -> f64 {
        let temp_score = ((temp - 5.0) / 35.0).clamp(0.0, 1.0);
        let hum_score = (hum / 100.0).clamp(0.0, 1.0);
        let ph_deviation = ((ph - 7.0).abs() / 5.0).clamp(0.0, 1.0);
        let cl_score = (chloride / 200.0).clamp(0.0, 1.0);

        temp_score * 0.2 + hum_score * 0.3 + ph_deviation * 0.2 + cl_score * 0.3
    }
}
