mod app;
#[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
pub mod asset;
#[cfg(not(target_arch = "wasm32"))]
mod remote_panel;
mod scene;
mod textures;
mod units;

pub use app::App;
#[cfg(not(target_arch = "wasm32"))]
pub use app::run;
#[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
pub use app::run_asset;
