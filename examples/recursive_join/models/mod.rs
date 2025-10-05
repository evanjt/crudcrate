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

// Re-export active models for database operations
pub use customer::ActiveModel as CustomerActiveModel;
pub use maintenance_record::ActiveModel as MaintenanceRecordActiveModel;
pub use vehicle::ActiveModel as VehicleActiveModel;
pub use vehicle_part::ActiveModel as VehiclePartActiveModel;

// Re-export CRUD types for API operations
// Temporarily commented out due to compilation issues
pub use customer::{Customer, CustomerCreate, CustomerUpdate};
// pub use maintenance_record::{MaintenanceRecord, MaintenanceRecordCreate, MaintenanceRecordUpdate};
// pub use vehicle::{Vehicle, VehicleCreate, VehicleUpdate};
// pub use vehicle_part::{VehiclePart, VehiclePartCreate, VehiclePartUpdate};