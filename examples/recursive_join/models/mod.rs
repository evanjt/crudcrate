// Order modules to respect dependencies: Customer depends on Vehicle, Vehicle depends on VehiclePart/MaintenanceRecord
pub mod customer;
pub mod maintenance_record;
pub mod vehicle;
pub mod vehicle_part;
