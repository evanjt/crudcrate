pub mod customer;
pub mod maintenance_record;
pub mod vehicle;
pub mod vehicle_part;

// Re-export entities for database operations
pub use customer::Entity as CustomerEntity;
pub use maintenance_record::Entity as MaintenanceRecordEntity;
pub use vehicle::Entity as VehicleEntity;
pub use vehicle_part::Entity as VehiclePartEntity;

// Re-export columns for queries
pub use customer::Column as CustomerColumn;
pub use maintenance_record::Column as MaintenanceRecordColumn;
pub use vehicle::Column as VehicleColumn;
pub use vehicle_part::Column as VehiclePartColumn;

// Re-export CRUD types for API operations
pub use customer::{Customer, CustomerResponse, CustomerList};
pub use maintenance_record::{MaintenanceRecord, MaintenanceRecordResponse, MaintenanceRecordList};
pub use vehicle::{Vehicle, VehicleResponse, VehicleList};
pub use vehicle_part::{VehiclePart, VehiclePartResponse, VehiclePartList};