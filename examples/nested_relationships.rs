//! Nested Relationships with use_target_models
//!
//! This example demonstrates how to handle complex nested relationships using the
//! `use_target_models` attribute, allowing Create and Update models to accept nested
//! entity structures instead of just foreign key IDs.
//!
//! ## Run the Example
//!
//! ```bash
//! cargo run --example nested_relationships --features derive
//! ```
//!
//! ## Key Features Demonstrated
//!
//! - `use_target_models` attribute for nested model composition
//! - Custom `fn_create` with nested entity creation
//! - Custom `fn_update` with nested entity updates
//! - `default = vec![]` for non-DB Vec fields
//! - Transaction management for atomic multi-table operations

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels, traits::MergeIntoActiveModel};
use sea_orm::{
    ActiveValue::Set, Database, DatabaseConnection, entity::prelude::*, TransactionTrait,
};
use uuid::Uuid;

// ============================================================================
// Sample Entity (Parent)
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "samples")]
#[crudcrate(
    api_struct = "Sample",
    name_singular = "sample",
    name_plural = "samples",
    description = "Environmental samples with treatments",
    generate_router,
    fn_create = create_sample_with_treatments,
    fn_update = update_sample_with_treatments,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable, fulltext)]
    pub name: String,

    #[crudcrate(sortable, filterable)]
    pub collected_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = Utc::now(), sortable)]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = Utc::now(), on_update = Utc::now(), sortable)]
    pub last_updated: DateTime<Utc>,

    // ⭐ KEY FEATURE: use_target_models allows SampleCreate to accept Vec<Treatment>
    // instead of Vec<TreatmentCreate>, making the API more intuitive
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, default = vec![], use_target_models)]
    pub treatments: Vec<Treatment>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "treatment::Entity")]
    Treatments,
}

impl Related<treatment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Treatments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Treatment Entity (Child)
// ============================================================================

pub mod treatment {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "treatments")]
    #[crudcrate(
        api_struct = "Treatment",
        name_singular = "treatment",
        name_plural = "treatments",
        generate_router
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub sample_id: Option<Uuid>,

        #[crudcrate(sortable, filterable)]
        pub treatment_type: String,

        #[crudcrate(sortable)]
        pub temperature_celsius: Option<f64>,

        #[crudcrate(exclude(create, update), on_create = Utc::now())]
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::SampleId",
            to = "super::Column::Id"
        )]
        Sample,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Sample.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================================
// Custom CRUD Functions with Nested Entity Handling
// ============================================================================

/// Custom create function that handles nested treatments
async fn create_sample_with_treatments(
    db: &DatabaseConnection,
    create_data: SampleCreate,
) -> Result<Sample, DbErr> {
    // Begin transaction for atomic operation
    let txn = db.begin().await?;

    // Extract treatments before creating sample (they're not in the DB model)
    let treatments_to_create = if create_data.treatments.is_empty() {
        None
    } else {
        Some(create_data.treatments.clone())
    };

    // Create the main sample entity
    let active_model: ActiveModel = create_data.into();
    let inserted = active_model.insert(&txn).await?;
    let sample_id = inserted.id;

    // Create related treatments if provided
    if let Some(treatments) = treatments_to_create {
        for treatment_create in treatments {
            let mut treatment_with_sample = treatment_create;
            treatment_with_sample.sample_id = Some(sample_id);

            // Use the generated CRUDResource trait to create treatments
            let _ = Treatment::create(&txn, treatment_with_sample).await?;
        }
    }

    // Commit transaction
    txn.commit().await?;

    // Return the complete sample with treatments loaded
    Sample::get_one(db, sample_id).await
}

/// Custom update function that handles nested treatment modifications
async fn update_sample_with_treatments(
    db: &DatabaseConnection,
    id: Uuid,
    update_data: SampleUpdate,
) -> Result<Sample, DbErr> {
    let txn = db.begin().await?;

    // Extract treatments from update data
    let treatments_to_update = Some(update_data.treatments.clone());

    // Update the sample using merge pattern (avoids infinite recursion)
    let existing_model = Entity::find_by_id(id)
        .one(&txn)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Sample not found".to_string()))?;

    let existing_active: ActiveModel = existing_model.into_active_model();
    let updated_active_model = update_data.merge_into_activemodel(existing_active)?;
    let _updated_sample = updated_active_model.update(&txn).await?;

    // Handle treatment updates: create new, update existing, delete missing
    if let Some(treatments) = treatments_to_update {
        // Get current treatments for this sample
        let current_treatments = treatment::Entity::find()
            .filter(treatment::Column::SampleId.eq(id))
            .all(&txn)
            .await?;
        let current_treatment_ids: Vec<Uuid> = current_treatments.iter().map(|t| t.id).collect();

        let mut updated_treatment_ids = Vec::new();

        for treatment_update in treatments {
            if let Some(Some(treatment_id)) = treatment_update.id {
                // Update existing treatment
                let existing_treatment = treatment::Entity::find_by_id(treatment_id)
                    .one(&txn)
                    .await?
                    .ok_or_else(|| DbErr::RecordNotFound("Treatment not found".to_string()))?;

                let existing_treatment_active = existing_treatment.into_active_model();
                let updated_treatment_active =
                    treatment_update.merge_into_activemodel(existing_treatment_active)?;
                let _ = updated_treatment_active.update(&txn).await?;
                updated_treatment_ids.push(treatment_id);
            } else {
                // Create new treatment
                let new_treatment_create = treatment::TreatmentCreate {
                    sample_id: Some(id),
                    treatment_type: treatment_update.treatment_type.flatten().unwrap_or_default(),
                    temperature_celsius: treatment_update.temperature_celsius.flatten(),
                };
                let new_treatment = Treatment::create(&txn, new_treatment_create).await?;
                updated_treatment_ids.push(new_treatment.id);
            }
        }

        // Delete treatments that are no longer in the update list
        let treatments_to_delete: Vec<Uuid> = current_treatment_ids
            .into_iter()
            .filter(|id| !updated_treatment_ids.contains(id))
            .collect();

        if !treatments_to_delete.is_empty() {
            treatment::Entity::delete_many()
                .filter(treatment::Column::Id.is_in(treatments_to_delete))
                .exec(&txn)
                .await?;
        }
    }

    txn.commit().await?;

    // Return updated sample with treatments
    Sample::get_one(db, id).await
}

// ============================================================================
// Database Setup
// ============================================================================

async fn setup_database() -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let db = Database::connect("sqlite::memory:").await?;

    // Create samples table
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE samples (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            collected_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_updated TEXT NOT NULL
        )"#
        .to_owned(),
    ))
    .await?;

    // Create treatments table
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE treatments (
            id TEXT PRIMARY KEY NOT NULL,
            sample_id TEXT,
            treatment_type TEXT NOT NULL,
            temperature_celsius REAL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (sample_id) REFERENCES samples(id)
        )"#
        .to_owned(),
    ))
    .await?;

    Ok(db)
}

// ============================================================================
// Example Usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Nested Relationships Example with use_target_models\n");

    let db = setup_database().await?;

    // ========================================================================
    // CREATE: Sample with nested treatments
    // ========================================================================
    println!("1️⃣  Creating sample with 2 treatments...");

    let create_sample = SampleCreate {
        name: "Soil Sample A".to_string(),
        collected_at: Utc::now(),
        // ⭐ Because of use_target_models, we can pass Vec<TreatmentCreate> directly!
        treatments: vec![
            treatment::TreatmentCreate {
                sample_id: None, // Will be set by custom fn_create
                treatment_type: "Heat".to_string(),
                temperature_celsius: Some(60.0),
            },
            treatment::TreatmentCreate {
                sample_id: None,
                treatment_type: "Chemical".to_string(),
                temperature_celsius: None,
            },
        ],
    };

    let sample = Sample::create(&db, create_sample).await?;
    println!("   ✅ Created sample: {} (ID: {})", sample.name, sample.id);
    println!("   ✅ With {} treatments", sample.treatments.len());
    for treatment in &sample.treatments {
        println!("      - {}: {:?}°C", treatment.treatment_type, treatment.temperature_celsius);
    }

    // ========================================================================
    // UPDATE: Modify treatments (add one, update one, delete one)
    // ========================================================================
    println!("\n2️⃣  Updating sample (add treatment, update existing)...");

    let first_treatment_id = sample.treatments[0].id;
    let update_sample = SampleUpdate {
        name: Some(Some("Soil Sample A - Updated".to_string())),
        collected_at: None,
        treatments: vec![
            // Keep and update first treatment
            treatment::TreatmentUpdate {
                id: Some(Some(first_treatment_id)),
                sample_id: None,
                treatment_type: Some(Some("Heat (Extended)".to_string())),
                temperature_celsius: Some(Some(75.0)), // Increased temperature
            },
            // Add new treatment (no ID means create)
            treatment::TreatmentUpdate {
                id: None,
                sample_id: None,
                treatment_type: Some(Some("UV Light".to_string())),
                temperature_celsius: None,
            },
            // Second treatment omitted - will be deleted
        ],
    };

    let updated_sample = Sample::update(&db, sample.id, update_sample).await?;
    println!("   ✅ Updated sample: {}", updated_sample.name);
    println!("   ✅ Now has {} treatments", updated_sample.treatments.len());
    for treatment in &updated_sample.treatments {
        println!("      - {}: {:?}°C", treatment.treatment_type, treatment.temperature_celsius);
    }

    println!("\n✅ Example complete!");
    println!("\n💡 Key Takeaways:");
    println!("   • use_target_models lets you pass Vec<T> in Create/Update models");
    println!("   • Custom fn_create/fn_update handle nested entity logic");
    println!("   • Transactions ensure atomic multi-table operations");
    println!("   • .flatten() pattern handles Option<Option<T>> in Updates");

    Ok(())
}
