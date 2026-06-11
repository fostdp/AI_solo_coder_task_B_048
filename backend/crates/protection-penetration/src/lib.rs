pub mod fick_diffusion;
pub mod materials;
pub mod thread_pool;

pub use fick_diffusion::{PenetrationSimulator, PenetrationResult, PenetrationProfile};
pub use materials::{ProtectiveMaterial, MaterialType, get_material, all_materials};
pub use thread_pool::{PenetrationThreadPool, SimulationRequest};
