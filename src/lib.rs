//! Trash management functionality

pub mod cli;
pub mod trash;
pub mod metadata;

// Re-export commonly used items
pub use cli::run;
