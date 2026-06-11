pub mod random_forest;
pub mod microbiome_data;

pub use random_forest::{MicrobeCorrelationAnalyzer, CorrelationResult, FeatureImportance};
pub use microbiome_data::{MicrobiomeSample, MicrobeAbundance, FunctionalGene, default_microbe_dataset};
