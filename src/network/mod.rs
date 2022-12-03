//! network module
use serde::{Deserialize, Serialize};

pub mod command;
pub mod controller;
pub mod event;
pub mod peer;

/// Network Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// Network Controller Configuration
    pub controller: controller::Controller,
}
