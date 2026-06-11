pub mod fick_diffusion;
pub mod materials;

pub use fick_diffusion::{PenetrationSimulator, PenetrationResult, PenetrationProfile};
pub use materials::{ProtectiveMaterial, MaterialType};
