/*
 * models/customer.rs — Customer entity
 *
 * Ported from: Universal EDA/Models/Customer.cs
 */

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub customer_name: String,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Customer {
    pub fn new(customer_name: String) -> Self {
        Customer {
            id: Uuid::new_v4(),
            customer_name,
            contact_email: None,
            contact_phone: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}
