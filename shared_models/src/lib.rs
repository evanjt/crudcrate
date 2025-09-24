pub mod customer;
pub mod vehicle;
pub mod vehicle_part;
pub mod maintenance_record;
// pub mod category;  // This triggers cyclic dependency warning - which is the test case
// pub mod category_with_depth;

// Re-export entities and models with prefixes to avoid conflicts
pub use customer::{Entity as CustomerEntity, Model as CustomerModel, ActiveModel as CustomerActiveModel, Column as CustomerColumn, Relation as CustomerRelation};
pub use vehicle::{Entity as VehicleEntity, Model as VehicleModel, ActiveModel as VehicleActiveModel, Column as VehicleColumn, Relation as VehicleRelation};
pub use vehicle_part::{Entity as VehiclePartEntity, Model as VehiclePartModel, ActiveModel as VehiclePartActiveModel, Column as VehiclePartColumn, Relation as VehiclePartRelation};
pub use maintenance_record::{Entity as MaintenanceRecordEntity, Model as MaintenanceRecordModel, ActiveModel as MaintenanceRecordActiveModel, Column as MaintenanceRecordColumn, Relation as MaintenanceRecordRelation};
// pub use category::{Entity as CategoryEntity, Model as CategoryModel, ActiveModel as CategoryActiveModel, Column as CategoryColumn, Relation as CategoryRelation};
// pub use category_with_depth::{Entity as CategoryWithDepthEntity, Model as CategoryWithDepthModel, ActiveModel as CategoryWithDepthActiveModel, Column as CategoryWithDepthColumn, Relation as CategoryWithDepthRelation};

// Re-export generated CRUD types (these should be unique per entity)
pub use customer::{Customer, CustomerCreate, CustomerUpdate};
pub use vehicle::{Vehicle, VehicleCreate, VehicleUpdate};
pub use vehicle_part::{VehiclePart, VehiclePartCreate, VehiclePartUpdate};
pub use maintenance_record::{MaintenanceRecord, MaintenanceRecordCreate, MaintenanceRecordUpdate};
// pub use category::{Category, CategoryCreate, CategoryUpdate};
// pub use category_with_depth::{CategoryWithDepth, CategoryWithDepthCreate, CategoryWithDepthUpdate};