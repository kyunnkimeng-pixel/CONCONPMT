pub mod ai;
mod ai_activation;
pub(crate) mod ai_candidate_normalization;
pub(crate) mod ai_grid;
pub(crate) mod ai_grid_retention;
#[cfg(test)]
mod ai_grid_retention_tests;
pub(crate) mod ai_handoff;
#[cfg(test)]
mod ai_handoff_integration_tests;
#[cfg(test)]
mod ai_handoff_maintenance_tests;
#[cfg(test)]
mod ai_handoff_safety_tests;
#[cfg(test)]
mod ai_integration_tests;
mod ai_managed_artifacts;
mod ai_new_icon;
pub(crate) mod ai_snapshots;
mod clone_artifacts;
#[cfg(test)]
mod collection_clone_tests;
pub mod collections;
pub mod editor;
pub mod effects;
pub mod export_profiles;
pub mod icons;
pub mod imports;
pub mod library;
pub mod motion;
pub mod motion_editor;
#[cfg(test)]
mod motion_editor_tests;
pub mod optimization;
pub mod settings;
pub mod source_files;
