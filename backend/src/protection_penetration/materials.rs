use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialType {
    #[serde(rename = "有机硅")]
    Silicone,
    #[serde(rename = "氟聚合物")]
    Fluoropolymer,
    #[serde(rename = "丙烯酸酯")]
    Acrylate,
    #[serde(rename = "环氧树脂")]
    Epoxy,
    #[serde(rename = "石蜡")]
    Paraffin,
    #[serde(rename = "纳米SiO2")]
    NanoSiO2,
}

impl MaterialType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MaterialType::Silicone => "有机硅",
            MaterialType::Fluoropolymer => "氟聚合物",
            MaterialType::Acrylate => "丙烯酸酯",
            MaterialType::Epoxy => "环氧树脂",
            MaterialType::Paraffin => "石蜡",
            MaterialType::NanoSiO2 => "纳米SiO2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectiveMaterial {
    pub name: MaterialType,
    pub diffusion_coefficient: f64,
    pub molecular_weight: f64,
    pub viscosity: f64,
    pub surface_tension: f64,
    pub solid_content: f64,
    pub optimal_temp: f64,
    pub description: &'static str,
}

pub fn silicone_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Silicone,
        diffusion_coefficient: 5.2e-10,
        molecular_weight: 850.0,
        viscosity: 15.0,
        surface_tension: 22.5,
        solid_content: 20.0,
        optimal_temp: 20.0,
        description: "甲基三乙氧基硅烷类，低粘度高渗透性，适合铁器和铜器",
    }
}

pub fn silicone_modified() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Silicone,
        diffusion_coefficient: 3.8e-10,
        molecular_weight: 1200.0,
        viscosity: 35.0,
        surface_tension: 24.0,
        solid_content: 30.0,
        optimal_temp: 22.0,
        description: "改性有机硅，附着力更强，渗透性略低",
    }
}

pub fn fluoropolymer_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Fluoropolymer,
        diffusion_coefficient: 2.5e-10,
        molecular_weight: 1500.0,
        viscosity: 25.0,
        surface_tension: 18.0,
        solid_content: 15.0,
        optimal_temp: 20.0,
        description: "全氟聚醚类，超强疏水性，耐候性极佳",
    }
}

pub fn acrylate_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Acrylate,
        diffusion_coefficient: 4.5e-10,
        molecular_weight: 600.0,
        viscosity: 12.0,
        surface_tension: 28.0,
        solid_content: 25.0,
        optimal_temp: 18.0,
        description: " Paraloid B72类，博物馆常用，可逆性好",
    }
}

pub fn epoxy_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Epoxy,
        diffusion_coefficient: 1.2e-10,
        molecular_weight: 2500.0,
        viscosity: 80.0,
        surface_tension: 42.0,
        solid_content: 60.0,
        optimal_temp: 25.0,
        description: "双组份环氧树脂，结构补强用，不可逆",
    }
}

pub fn paraffin_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::Paraffin,
        diffusion_coefficient: 8.5e-10,
        molecular_weight: 350.0,
        viscosity: 8.0,
        surface_tension: 30.0,
        solid_content: 50.0,
        optimal_temp: 60.0,
        description: "微晶石蜡，加热浸渍，传统保护方法",
    }
}

pub fn nano_sio2_standard() -> ProtectiveMaterial {
    ProtectiveMaterial {
        name: MaterialType::NanoSiO2,
        diffusion_coefficient: 6.8e-10,
        molecular_weight: 60.1,
        viscosity: 5.0,
        surface_tension: 20.0,
        solid_content: 10.0,
        optimal_temp: 20.0,
        description: "纳米二氧化硅溶胶，超疏水涂层，渗透性强",
    }
}

pub fn get_material(material: MaterialType) -> ProtectiveMaterial {
    match material {
        MaterialType::Silicone => silicone_standard(),
        MaterialType::Fluoropolymer => fluoropolymer_standard(),
        MaterialType::Acrylate => acrylate_standard(),
        MaterialType::Epoxy => epoxy_standard(),
        MaterialType::Paraffin => paraffin_standard(),
        MaterialType::NanoSiO2 => nano_sio2_standard(),
    }
}

pub fn all_materials() -> Vec<ProtectiveMaterial> {
    vec![
        silicone_standard(),
        fluoropolymer_standard(),
        acrylate_standard(),
        epoxy_standard(),
        paraffin_standard(),
        nano_sio2_standard(),
    ]
}
