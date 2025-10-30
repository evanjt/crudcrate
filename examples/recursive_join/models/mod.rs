// Order modules to respect dependencies: Customer depends on Vehicle, Vehicle depends on VehiclePart/MaintenanceRecord
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

// Re-export active models for database operations

// Re-export CRUD types for API operations
pub use customer::Customer;
pub use vehicle::Vehicle;
