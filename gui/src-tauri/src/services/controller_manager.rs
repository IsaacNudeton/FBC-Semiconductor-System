//! services/controller_manager.rs — Controller Manager
//!
//! Ported from: Universal EDA/Services/ControllerManager.cs
//!
//! Manages multiple controller connections (SSH + raw Ethernet).
//! Uses DatabaseService for persistence.

use crate::models::controller::{Controller, ControllerStatus};
use crate::services::database_service::DatabaseService;
use crate::ssh::SshSessionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;
use serde_json::Value;

/// Controller Manager — manages fleet of controllers
pub struct ControllerManager {
    controllers: Arc<RwLock<HashMap<Uuid, Controller>>>,
    ssh: Arc<SshSessionManager>,
    db: Arc<DatabaseService>,
}

impl ControllerManager {
    pub fn new(db_path: &str, ssh: Arc<SshSessionManager>) -> Result<Self, String> {
        let db = DatabaseService::open(db_path)?;

        Ok(ControllerManager {
            controllers: Arc::new(RwLock::new(HashMap::new())),
            ssh,
            db: Arc::new(db),
        })
    }

    /// Load controllers from database into memory cache
    pub async fn load_from_db(&self) -> Result<(), String> {
        let controllers = self.db.get_controllers().await?;
        
        let mut cache = self.controllers.write().await;
        for c in controllers {
            cache.insert(c.id, c);
        }
        
        info!("Loaded {} controllers from database", cache.len());
        Ok(())
    }

    /// Add a controller to the fleet (and database)
    pub async fn add_controller(&self, controller: Controller) -> Result<(), String> {
        /* Insert into database */
        self.db.insert_controller(controller.clone()).await?;

        /* Add to memory cache */
        let mut controllers = self.controllers.write().await;
        controllers.insert(controller.id, controller);
        info!("Controller added to fleet");
        Ok(())
    }

    /// Remove a controller from the fleet
    pub async fn remove_controller(&self, id: Uuid) -> Result<(), String> {
        /* TODO: Remove from database */
        
        let mut controllers = self.controllers.write().await;
        controllers.remove(&id);
        info!("Controller removed from fleet");
        Ok(())
    }

    /// Get a controller by ID
    pub async fn get_controller(&self, id: Uuid) -> Option<Controller> {
        let controllers = self.controllers.read().await;
        controllers.get(&id).cloned()
    }

    /// Get all controllers
    pub async fn get_all_controllers(&self) -> Vec<Controller> {
        let controllers = self.controllers.read().await;
        controllers.values().cloned().collect()
    }

    /// Get fleet status as JSON
    pub async fn get_fleet_status(&self) -> Result<Value, String> {
        self.db.get_fleet_status().await
    }

    /// Connect to a controller via SSH (uses SshSessionManager from ssh.rs)
    pub async fn connect_to_controller(
        &self,
        id: Uuid,
        username: &str,
        password: &str,
        app_handle: tauri::AppHandle,
    ) -> Result<u32, String> {
        let controller = {
            let controllers = self.controllers.read().await;
            controllers.get(&id).cloned().ok_or_else(|| "Controller not found".to_string())?
        };

        info!("Connecting to controller {} at {}", controller.id, controller.ip_address);

        let session_id = self.ssh.connect(
            controller.ip_address.clone(),
            22,
            username.to_string(),
            password.to_string(),
            app_handle,
        ).await?;

        let mut controllers = self.controllers.write().await;
        if let Some(c) = controllers.get_mut(&id) {
            c.mark_online();
        }

        Ok(session_id)
    }

    /// Disconnect from a controller
    pub async fn disconnect_from_controller(&self, session_id: u32, id: Uuid) {
        let _ = self.ssh.disconnect(session_id).await;

        let mut controllers = self.controllers.write().await;
        if let Some(c) = controllers.get_mut(&id) {
            c.mark_offline();
        }
    }

    /// Scan network for controllers (simple ping sweep)
    pub async fn scan_network(&self, network: &str) -> Vec<Controller> {
        info!("Scanning network {} for controllers", network);

        /* TODO: Implement actual network scan */
        Vec::new()
    }

    /// Get online controller count
    pub async fn get_online_count(&self) -> usize {
        let controllers = self.controllers.read().await;
        controllers.values()
            .filter(|c| c.status == ControllerStatus::Online)
            .count()
    }

    /// Get total controller count
    pub async fn get_total_count(&self) -> usize {
        let controllers = self.controllers.read().await;
        controllers.len()
    }
}
