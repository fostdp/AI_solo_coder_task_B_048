use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrobeAbundance {
    pub taxon_id: String,
    pub taxon_name: String,
    pub taxon_rank: TaxonRank,
    pub relative_abundance: f64,
    pub otu_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxonRank {
    Kingdom,
    Phylum,
    Class,
    Order,
    Family,
    Genus,
    Species,
}

impl TaxonRank {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaxonRank::Kingdom => "界",
            TaxonRank::Phylum => "门",
            TaxonRank::Class => "纲",
            TaxonRank::Order => "目",
            TaxonRank::Family => "科",
            TaxonRank::Genus => "属",
            TaxonRank::Species => "种",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionalGene {
    pub gene_name: String,
    pub function: String,
    pub category: GeneCategory,
    pub relative_expression: f64,
    pub corrosion_relevance: CorrosionRelevance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneCategory {
    AcidProduction,
    SulfurOxidation,
    IronReduction,
    IronOxidation,
    EPSProduction,
    AntibioticResistance,
    Other,
}

impl GeneCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            GeneCategory::AcidProduction => "产酸相关",
            GeneCategory::SulfurOxidation => "硫氧化",
            GeneCategory::IronReduction => "铁还原",
            GeneCategory::IronOxidation => "铁氧化",
            GeneCategory::EPSProduction => "胞外聚合物(EPS)",
            GeneCategory::AntibioticResistance => "抗生素抗性",
            GeneCategory::Other => "其他",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrosionRelevance {
    StrongPromote,
    ModeratePromote,
    WeakPromote,
    Neutral,
    WeakInhibit,
    ModerateInhibit,
    StrongInhibit,
}

impl CorrosionRelevance {
    pub fn as_str(&self) -> &'static str {
        match self {
            CorrosionRelevance::StrongPromote => "强促腐蚀",
            CorrosionRelevance::ModeratePromote => "中促腐蚀",
            CorrosionRelevance::WeakPromote => "弱促腐蚀",
            CorrosionRelevance::Neutral => "中性",
            CorrosionRelevance::WeakInhibit => "弱抑腐蚀",
            CorrosionRelevance::ModerateInhibit => "中抑腐蚀",
            CorrosionRelevance::StrongInhibit => "强抑腐蚀",
        }
    }

    pub fn numeric(&self) -> f64 {
        match self {
            CorrosionRelevance::StrongPromote => 1.0,
            CorrosionRelevance::ModeratePromote => 0.6,
            CorrosionRelevance::WeakPromote => 0.2,
            CorrosionRelevance::Neutral => 0.0,
            CorrosionRelevance::WeakInhibit => -0.2,
            CorrosionRelevance::ModerateInhibit => -0.6,
            CorrosionRelevance::StrongInhibit => -1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrobiomeSample {
    pub sample_id: String,
    pub zone: String,
    pub sensor_id: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub sampling_depth_cm: f64,
    pub temperature_c: f64,
    pub ph: f64,
    pub moisture_pct: f64,
    pub total_organic_carbon_pct: f64,
    pub microbial_biomass_cfu_g: f64,
    pub shannon_diversity: f64,
    pub simpson_diversity: f64,
    pub evenness: f64,
    pub taxa: Vec<MicrobeAbundance>,
    pub functional_genes: Vec<FunctionalGene>,
    pub corrosion_rate_observed: f64,
    pub chloride_ppm: f64,
    pub timestamp: Option<String>,
}

impl MicrobiomeSample {
    pub fn corrosion_gene_score(&self) -> f64 {
        let mut score = 0.0;
        for g in &self.functional_genes {
            score += g.relative_expression * g.corrosion_relevance.numeric();
        }
        score
    }

    pub fn dominant_taxa(&self, rank: TaxonRank, top_n: usize) -> Vec<&MicrobeAbundance> {
        let mut filtered: Vec<&MicrobeAbundance> = self
            .taxa
            .iter()
            .filter(|t| t.taxon_rank == rank)
            .collect();
        filtered.sort_by(|a, b| {
            b.relative_abundance
                .partial_cmp(&a.relative_abundance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(top_n);
        filtered
    }
}

pub fn generate_mock_microbiome(sample_id: &str, zone: &str, lat: f64, lng: f64, seed: u64) -> MicrobiomeSample {
    use rand::{rngs::StdRng, SeedableRng, Rng};
    let mut rng = StdRng::seed_from_u64(seed);

    let base_ph = 6.5 + rng.gen_range(-1.5..1.5);
    let base_temp = 16.0 + rng.gen_range(-4.0..4.0);
    let base_moisture = 45.0 + rng.gen_range(-20.0..20.0);
    let base_toc = 1.2 + rng.gen_range(-0.6..0.8);
    let base_biomass = 1e6 + rng.gen_range(0.0..5e6);
    let base_chloride = 50.0 + rng.gen_range(-30.0..80.0);
    let base_corrosion = 0.15 + rng.gen_range(0.0..0.4);

    let taxa = vec![
        MicrobeAbundance {
            taxon_id: "T001".to_string(),
            taxon_name: "变形菌门 (Proteobacteria)".to_string(),
            taxon_rank: TaxonRank::Phylum,
            relative_abundance: 35.0 + rng.gen_range(-10.0..10.0),
            otu_count: 1250,
        },
        MicrobeAbundance {
            taxon_id: "T002".to_string(),
            taxon_name: "放线菌门 (Actinobacteria)".to_string(),
            taxon_rank: TaxonRank::Phylum,
            relative_abundance: 20.0 + rng.gen_range(-8.0..8.0),
            otu_count: 890,
        },
        MicrobeAbundance {
            taxon_id: "T003".to_string(),
            taxon_name: "厚壁菌门 (Firmicutes)".to_string(),
            taxon_rank: TaxonRank::Phylum,
            relative_abundance: 18.0 + rng.gen_range(-6.0..6.0),
            otu_count: 720,
        },
        MicrobeAbundance {
            taxon_id: "T004".to_string(),
            taxon_name: "拟杆菌门 (Bacteroidetes)".to_string(),
            taxon_rank: TaxonRank::Phylum,
            relative_abundance: 12.0 + rng.gen_range(-5.0..5.0),
            otu_count: 480,
        },
        MicrobeAbundance {
            taxon_id: "T005".to_string(),
            taxon_name: "酸杆菌门 (Acidobacteria)".to_string(),
            taxon_rank: TaxonRank::Phylum,
            relative_abundance: 8.0 + rng.gen_range(-4.0..4.0),
            otu_count: 310,
        },
        MicrobeAbundance {
            taxon_id: "T006".to_string(),
            taxon_name: "铁氧化细菌 (Acidithiobacillus)".to_string(),
            taxon_rank: TaxonRank::Genus,
            relative_abundance: 2.5 + rng.gen_range(-1.5..3.0),
            otu_count: 95,
        },
        MicrobeAbundance {
            taxon_id: "T007".to_string(),
            taxon_name: "硫酸盐还原菌 (Desulfovibrio)".to_string(),
            taxon_rank: TaxonRank::Genus,
            relative_abundance: 3.2 + rng.gen_range(-2.0..2.5),
            otu_count: 128,
        },
        MicrobeAbundance {
            taxon_id: "T008".to_string(),
            taxon_name: "铁还原菌 (Shewanella)".to_string(),
            taxon_rank: TaxonRank::Genus,
            relative_abundance: 1.8 + rng.gen_range(-1.0..1.5),
            otu_count: 72,
        },
    ];

    let functional_genes = vec![
        FunctionalGene {
            gene_name: "soxY (硫氧化基因簇)".to_string(),
            function: "硫代硫酸盐氧化为硫酸".to_string(),
            category: GeneCategory::SulfurOxidation,
            relative_expression: 0.65 + rng.gen_range(-0.3..0.3),
            corrosion_relevance: CorrosionRelevance::StrongPromote,
        },
        FunctionalGene {
            gene_name: "dsrA (亚硫酸盐还原酶)".to_string(),
            function: "硫酸盐还原产生硫化氢".to_string(),
            category: GeneCategory::SulfurOxidation,
            relative_expression: 0.55 + rng.gen_range(-0.3..0.3),
            corrosion_relevance: CorrosionRelevance::StrongPromote,
        },
        FunctionalGene {
            gene_name: "mtrC (外膜细胞色素c)".to_string(),
            function: "胞外铁离子还原".to_string(),
            category: GeneCategory::IronReduction,
            relative_expression: 0.42 + rng.gen_range(-0.25..0.25),
            corrosion_relevance: CorrosionRelevance::ModeratePromote,
        },
        FunctionalGene {
            gene_name: "rus (铜蓝蛋白)".to_string(),
            function: "亚铁离子氧化".to_string(),
            category: GeneCategory::IronOxidation,
            relative_expression: 0.35 + rng.gen_range(-0.2..0.2),
            corrosion_relevance: CorrosionRelevance::StrongPromote,
        },
        FunctionalGene {
            gene_name: "pga (PGA合成酶)".to_string(),
            function: "胞外多糖EPS合成".to_string(),
            category: GeneCategory::EPSProduction,
            relative_expression: 0.48 + rng.gen_range(-0.25..0.25),
            corrosion_relevance: CorrosionRelevance::ModeratePromote,
        },
        FunctionalGene {
            gene_name: "ldh (乳酸脱氢酶)".to_string(),
            function: "有机酸产生".to_string(),
            category: GeneCategory::AcidProduction,
            relative_expression: 0.52 + rng.gen_range(-0.3..0.3),
            corrosion_relevance: CorrosionRelevance::ModeratePromote,
        },
        FunctionalGene {
            gene_name: "bphA (联苯双加氧酶)".to_string(),
            function: "芳香族化合物降解".to_string(),
            category: GeneCategory::Other,
            relative_expression: 0.18 + rng.gen_range(-0.1..0.15),
            corrosion_relevance: CorrosionRelevance::Neutral,
        },
        FunctionalGene {
            gene_name: "sodA (超氧化物歧化酶)".to_string(),
            function: "抗氧化保护，生物膜稳定".to_string(),
            category: GeneCategory::Other,
            relative_expression: 0.30 + rng.gen_range(-0.15..0.2),
            corrosion_relevance: CorrosionRelevance::WeakPromote,
        },
    ];

    let shannon = 2.8 + rng.gen_range(-0.8..0.8);

    MicrobiomeSample {
        sample_id: sample_id.to_string(),
        zone: zone.to_string(),
        sensor_id: None,
        latitude: lat,
        longitude: lng,
        sampling_depth_cm: 5.0 + rng.gen_range(-3.0..10.0),
        temperature_c: base_temp,
        ph: base_ph,
        moisture_pct: base_moisture.clamp(10.0, 90.0),
        total_organic_carbon_pct: base_toc.clamp(0.1, 5.0),
        microbial_biomass_cfu_g: base_biomass.max(1e5),
        shannon_diversity: shannon.max(0.5),
        simpson_diversity: 1.0 - 1.0 / (shannon.exp()).max(1.0),
        evenness: (shannon / (taxa.len() as f64).ln()).clamp(0.0, 1.0),
        taxa,
        functional_genes,
        corrosion_rate_observed: base_corrosion.max(0.01),
        chloride_ppm: base_chloride.max(5.0),
        timestamp: None,
    }
}

pub fn default_microbe_dataset() -> Vec<MicrobiomeSample> {
    let zones = ["区域1-主营区", "区域2-药房区", "区域3-伤兵区", "区域4-器械库", "区域5-手术区", "区域6-外围"];
    let center_lat = 34.2658;
    let center_lng = 108.9542;

    zones.iter().enumerate().map(|(i, zone)| {
        let angle = (i as f64) * 1.047;
        let radius = 0.0004 + (i as f64) * 0.0001;
        let lat = center_lat + radius * angle.cos();
        let lng = center_lng + radius * angle.sin();
        generate_mock_microbiome(
            &format!("MICRO-{:03}", i + 1),
            zone,
            lat,
            lng,
            (i as u64) * 12345 + 42,
        )
    }).collect()
}
