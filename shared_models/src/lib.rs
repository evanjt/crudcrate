pub mod customer;
pub mod maintenance_record;
pub mod vehicle;
pub mod vehicle_part;
// pub mod category;  // This triggers cyclic dependency warning - which is the test case
// pub mod category_with_depth;

// Re-export entities and models with prefixes to avoid conflicts
pub use customer::{
    ActiveModel as CustomerActiveModel, Column as CustomerColumn, Entity as CustomerEntity,
    Model as CustomerModel, Relation as CustomerRelation,
};
pub use maintenance_record::{
    ActiveModel as MaintenanceRecordActiveModel, Column as MaintenanceRecordColumn,
    Entity as MaintenanceRecordEntity, Model as MaintenanceRecordModel,
    Relation as MaintenanceRecordRelation,
};
pub use vehicle::{
    ActiveModel as VehicleActiveModel, Column as VehicleColumn, Entity as VehicleEntity,
    Model as VehicleModel, Relation as VehicleRelation,
};
pub use vehicle_part::{
    ActiveModel as VehiclePartActiveModel, Column as VehiclePartColumn,
    Entity as VehiclePartEntity, Model as VehiclePartModel, Relation as VehiclePartRelation,
};
// pub use category::{Entity as CategoryEntity, Model as CategoryModel, ActiveModel as CategoryActiveModel, Column as CategoryColumn, Relation as CategoryRelation};
// pub use category_with_depth::{Entity as CategoryWithDepthEntity, Model as CategoryWithDepthModel, ActiveModel as CategoryWithDepthActiveModel, Column as CategoryWithDepthColumn, Relation as CategoryWithDepthRelation};

// Re-export generated CRUD types (these should be unique per entity)
pub use customer::{Customer, CustomerCreate, CustomerUpdate};
pub use maintenance_record::{MaintenanceRecord, MaintenanceRecordCreate, MaintenanceRecordUpdate};
pub use vehicle::{Vehicle, VehicleCreate, VehicleUpdate};
pub use vehicle_part::{VehiclePart, VehiclePartCreate, VehiclePartUpdate};
// pub use category::{Category, CategoryCreate, CategoryUpdate};
// pub use category_with_depth::{CategoryWithDepth, CategoryWithDepthCreate, CategoryWithDepthUpdate};
