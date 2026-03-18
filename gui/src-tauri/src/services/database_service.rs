/*
 * services/database_service.rs — Database Service for LRM C Engine
 *
 * High-level service wrapping the LRM C database FFI.
 * This is what controllers, boards, LOTs actually use.
 */

use crate::database::lrm_ffi::{Database, QueryResult};
use crate::models::{Controller, Board, Lot, User};
use crate::models::controller::ControllerStatus;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

/// Database Service — high-level CRUD operations
pub struct DatabaseService {
    db: Arc<RwLock<Database>>,
}

impl DatabaseService {
    /* ═══════════════════════════════════════════════════════════════
     * LIFECYCLE
     * ═══════════════════════════════════════════════════════════════ */

    pub fn open(path: &str) -> Result<Self, String> {
        let db = Database::open(path)?;
        db.init_schema().map_err(|e| format!("Schema init failed: {}", e))?;
        
        info!("Database opened: {}", path);
        
        Ok(DatabaseService {
            db: Arc::new(RwLock::new(db)),
        })
    }

    /* ═══════════════════════════════════════════════════════════════
     * CONTROLLERS
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn get_controllers(&self) -> Result<Vec<Controller>, String> {
        let db = self.db.read().await;
        let result = db.get_controllers()?;
        
        /* Parse JSON result into Vec<Controller> */
        /* TODO: Implement JSON parsing with serde_json */
        
        Ok(vec![])  /* Placeholder */
    }

    pub async fn insert_controller(&self, controller: Controller) -> Result<(), String> {
        let db = self.db.write().await;
        
        db.insert_controller(
            &controller.id.to_string(),
            &controller.ip_address,
            &controller.mac_address,
            controller.status as i32,
            &controller.firmware_version,
        )?;
        
        info!("Controller inserted: {}", controller.id);
        Ok(())
    }

    pub async fn update_controller_status(&self, id: String, status: i32) -> Result<(), String> {
        /* TODO: Implement UPDATE query in LRM FFI */
        Ok(())
    }

    /* ═══════════════════════════════════════════════════════════════
     * BOARDS
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn get_boards(&self) -> Result<Vec<Board>, String> {
        let db = self.db.read().await;
        let result = db.get_boards()?;
        
        /* Parse JSON result into Vec<Board> */
        /* TODO: Implement JSON parsing */
        
        Ok(vec![])  /* Placeholder */
    }

    pub async fn get_boards_by_lot(&self, lot_id: String) -> Result<Vec<Board>, String> {
        let db = self.db.read().await;
        /* TODO: Implement get_boards_by_lot in FFI */
        Ok(vec![])
    }

    /* ═══════════════════════════════════════════════════════════════
     * LOTS
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn get_lots(&self) -> Result<Vec<Lot>, String> {
        let db = self.db.read().await;
        let result = db.get_lots()?;
        
        /* Parse JSON result into Vec<Lot> */
        /* TODO: Implement JSON parsing */
        
        Ok(vec![])  /* Placeholder */
    }

    pub async fn advance_lot(&self, lot_id: String) -> Result<(), String> {
        /* TODO: Wire to lrm_advance_lot FFI once exposed on Database */
        info!("LOT advance requested: {}", lot_id);
        Ok(())
    }

    /* ═══════════════════════════════════════════════════════════════
     * USERS
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn get_user(&self, username: String) -> Result<Option<User>, String> {
        /* TODO: Implement user queries */
        Ok(None)
    }

    pub async fn create_user(&self, user: User) -> Result<(), String> {
        /* TODO: Implement user insertion */
        Ok(())
    }

    /* ═══════════════════════════════════════════════════════════════
     * QUERIES
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn query(&self, sql: &str) -> Result<QueryResult, String> {
        let db = self.db.read().await;
        db.query(sql)
    }

    /* ═══════════════════════════════════════════════════════════════
     * FLEET STATUS (convenience method for frontend)
     * ═══════════════════════════════════════════════════════════════ */

    pub async fn get_fleet_status(&self) -> Result<Value, String> {
        use serde_json::json;
        
        let controllers = self.get_controllers().await?;
        let online = controllers.iter().filter(|c| c.status == ControllerStatus::Online).count();
        
        Ok(json!({
            "online": online,
            "total": controllers.len(),
            "controllers": controllers
        }))
    }
}

/* ═══════════════════════════════════════════════════════════════
 * TESTS
 * ═══════════════════════════════════════════════════════════════ */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Controller;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_database_service_open() {
        let service = DatabaseService::open("test_inventory.db");
        assert!(service.is_ok(), "Database should open");
    }

    #[tokio::test]
    async fn test_insert_controller() {
        let service = DatabaseService::open("test_inventory.db").unwrap();
        
        let controller = Controller::new(
            "192.168.1.100".to_string(),
            "AA:BB:CC:DD:EE:FF".to_string(),
        );
        
        let result = service.insert_controller(controller).await;
        assert!(result.is_ok(), "Controller should insert");
    }

    #[tokio::test]
    async fn test_get_fleet_status() {
        let service = DatabaseService::open("test_inventory.db").unwrap();
        
        let status = service.get_fleet_status().await;
        assert!(status.is_ok(), "Fleet status should query");
    }
}
