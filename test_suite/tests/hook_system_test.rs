//! Tests for the hook system
//!
//! Tests the hook syntax: `{operation}::{cardinality}::{phase}`
//!
//! Hook syntax:
//! - Operations: create, read, update, delete
//! - Cardinality: one (single item), many (batch)
//! - Phases: pre (before), body (replace), post (after)

use chrono::{DateTime, Utc};
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

// ============================================================================
// HOOK TRACKING - Static flags to verify hook execution
// ============================================================================

static CREATE_PRE_CALLED: AtomicBool = AtomicBool::new(false);
static CREATE_POST_CALLED: AtomicBool = AtomicBool::new(false);
static READ_PRE_CALLED: AtomicBool = AtomicBool::new(false);
static READ_POST_CALLED: AtomicBool = AtomicBool::new(false);
static UPDATE_PRE_CALLED: AtomicBool = AtomicBool::new(false);
static UPDATE_POST_CALLED: AtomicBool = AtomicBool::new(false);
static DELETE_PRE_CALLED: AtomicBool = AtomicBool::new(false);
static DELETE_POST_CALLED: AtomicBool = AtomicBool::new(false);

fn reset_hook_flags() {
    CREATE_PRE_CALLED.store(false, Ordering::SeqCst);
    CREATE_POST_CALLED.store(false, Ordering::SeqCst);
    READ_PRE_CALLED.store(false, Ordering::SeqCst);
    READ_POST_CALLED.store(false, Ordering::SeqCst);
    UPDATE_PRE_CALLED.store(false, Ordering::SeqCst);
    UPDATE_POST_CALLED.store(false, Ordering::SeqCst);
    DELETE_PRE_CALLED.store(false, Ordering::SeqCst);
    DELETE_POST_CALLED.store(false, Ordering::SeqCst);
}

// ============================================================================
// HOOK FUNCTIONS
// ============================================================================

/// Pre-create hook: validation before creating
async fn validate_before_create(
    _db: &sea_orm::DatabaseConnection,
    data: &HookTestItemCreate,
) -> Result<(), ApiError> {
    CREATE_PRE_CALLED.store(true, Ordering::SeqCst);

    // Validate name is not empty
    if data.name.is_empty() {
        return Err(ApiError::bad_request("Name cannot be empty"));
    }
    Ok(())
}

/// Post-create hook: side effects after creating
async fn notify_after_create(
    _db: &sea_orm::DatabaseConnection,
    _entity: &HookTestItem,
) -> Result<(), ApiError> {
    CREATE_POST_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Pre-read hook: authorization check
async fn check_read_permission(
    _db: &sea_orm::DatabaseConnection,
    _id: Uuid,
) -> Result<(), ApiError> {
    READ_PRE_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Post-read hook: enrich data after fetching
async fn enrich_after_read(
    _db: &sea_orm::DatabaseConnection,
    _entity: &HookTestItem,
) -> Result<(), ApiError> {
    READ_POST_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Pre-update hook: validation before updating
async fn validate_before_update(
    _db: &sea_orm::DatabaseConnection,
    _id: Uuid,
    data: &HookTestItemUpdate,
) -> Result<(), ApiError> {
    UPDATE_PRE_CALLED.store(true, Ordering::SeqCst);

    // Validate name if provided (double Option: outer for "provided", inner for actual value)
    if let Some(Some(ref name)) = data.name {
        if name.is_empty() {
            return Err(ApiError::bad_request("Name cannot be empty"));
        }
    }
    Ok(())
}

/// Post-update hook: audit log after updating
async fn audit_after_update(
    _db: &sea_orm::DatabaseConnection,
    _entity: &HookTestItem,
) -> Result<(), ApiError> {
    UPDATE_POST_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Pre-delete hook: cleanup before deleting
async fn cleanup_before_delete(
    _db: &sea_orm::DatabaseConnection,
    _id: Uuid,
) -> Result<(), ApiError> {
    DELETE_PRE_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Post-delete hook: notification after deleting
async fn notify_after_delete(
    _db: &sea_orm::DatabaseConnection,
    _id: Uuid,
) -> Result<(), ApiError> {
    DELETE_POST_CALLED.store(true, Ordering::SeqCst);
    Ok(())
}

// ============================================================================
// ENTITY WITH ALL HOOKS
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "hook_test_items")]
#[crudcrate(
    api_struct = "HookTestItem",
    name_singular = "hook_test_item",
    name_plural = "hook_test_items",
    // Create hooks
    create::one::pre = validate_before_create,
    create::one::post = notify_after_create,
    // Read hooks
    read::one::pre = check_read_permission,
    read::one::post = enrich_after_read,
    // Update hooks
    update::one::pre = validate_before_update,
    update::one::post = audit_after_update,
    // Delete hooks
    delete::one::pre = cleanup_before_delete,
    delete::one::post = notify_after_delete,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// TESTS - Verify hook syntax compiles and models are generated
// ============================================================================

/// Test that the new hook syntax compiles correctly.
/// If the macro expansion is incorrect, this test won't compile.
#[test]
fn test_hook_syntax_compiles() {
    // Verify the generated models exist and have expected fields
    let create_model = HookTestItemCreate {
        name: "test".to_string(),
    };
    assert_eq!(create_model.name, "test");

    // Update model has Option<Option<T>> for optional fields
    let update_model = HookTestItemUpdate {
        name: Some(Some("updated".to_string())),
    };
    assert!(update_model.name.is_some());

    // Verify list model exists
    let list_model = HookTestItemList {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        created_at: Utc::now(),
    };
    assert!(!list_model.name.is_empty());
}

/// Test that the hook functions exist and have been defined.
/// The actual signature verification happens at macro expansion time.
#[test]
fn test_hook_functions_defined() {
    // If these functions didn't exist or had wrong signatures,
    // the macro expansion would fail to compile
    assert!(true, "Hook functions are defined - macro compilation succeeded");
}

/// Test that the CRUDResource trait is implemented
#[test]
fn test_crud_resource_trait_implemented() {
    // Verify the trait constants are set
    assert_eq!(HookTestItem::RESOURCE_NAME_SINGULAR, "hook_test_item");
    assert_eq!(HookTestItem::RESOURCE_NAME_PLURAL, "hook_test_items");
    assert_eq!(HookTestItem::TABLE_NAME, "hook_test_items");
}

/// Test that hook flags are properly reset between tests
#[test]
fn test_hook_flag_reset() {
    // Set all flags
    CREATE_PRE_CALLED.store(true, Ordering::SeqCst);
    CREATE_POST_CALLED.store(true, Ordering::SeqCst);
    READ_PRE_CALLED.store(true, Ordering::SeqCst);

    // Reset
    reset_hook_flags();

    // Verify all are false
    assert!(!CREATE_PRE_CALLED.load(Ordering::SeqCst));
    assert!(!CREATE_POST_CALLED.load(Ordering::SeqCst));
    assert!(!READ_PRE_CALLED.load(Ordering::SeqCst));
    assert!(!READ_POST_CALLED.load(Ordering::SeqCst));
    assert!(!UPDATE_PRE_CALLED.load(Ordering::SeqCst));
    assert!(!UPDATE_POST_CALLED.load(Ordering::SeqCst));
    assert!(!DELETE_PRE_CALLED.load(Ordering::SeqCst));
    assert!(!DELETE_POST_CALLED.load(Ordering::SeqCst));
}

/// Test that create model excludes auto-generated fields
#[test]
fn test_create_model_excludes_auto_fields() {
    // CreateModel should only have 'name' field, not 'id' or 'created_at'
    let model = HookTestItemCreate {
        name: "test".to_string(),
    };

    // This compiles only if the struct has exactly this shape
    let HookTestItemCreate { name } = model;
    assert_eq!(name, "test");
}

/// Test that update model has all optional fields
#[test]
fn test_update_model_all_optional() {
    // Update model should have Option<Option<T>> for each field
    let model = HookTestItemUpdate { name: None };
    assert!(model.name.is_none());

    let model_with_value = HookTestItemUpdate {
        name: Some(Some("new name".to_string())),
    };
    assert_eq!(model_with_value.name, Some(Some("new name".to_string())));
}

// ============================================================================
// INTEGRATION TESTS - Verify hooks are actually called during CRUD operations
// ============================================================================

mod integration {
    use super::*;
    use crudcrate::CRUDResource;
    use sea_orm::{Database, DatabaseConnection};
    use serial_test::serial;

    async fn setup_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
        let db = Database::connect("sqlite::memory:").await?;

        db.execute(sea_orm::Statement::from_string(
            db.get_database_backend(),
            r"CREATE TABLE hook_test_items (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
            .to_owned(),
        ))
        .await?;

        Ok(db)
    }

    #[tokio::test]
    #[serial]
    async fn test_create_hooks_called() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Verify hooks not called yet
        assert!(!CREATE_PRE_CALLED.load(Ordering::SeqCst));
        assert!(!CREATE_POST_CALLED.load(Ordering::SeqCst));

        // Create an item using the CRUDResource trait
        let create_data = HookTestItemCreate {
            name: "test item".to_string(),
        };
        let result = HookTestItem::create(&db, create_data).await;
        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        // Verify pre and post hooks were called
        assert!(
            CREATE_PRE_CALLED.load(Ordering::SeqCst),
            "create::one::pre hook should have been called"
        );
        assert!(
            CREATE_POST_CALLED.load(Ordering::SeqCst),
            "create::one::post hook should have been called"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_create_pre_hook_validation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Try to create with empty name - should fail validation in pre hook
        let create_data = HookTestItemCreate {
            name: "".to_string(),
        };
        let result = HookTestItem::create(&db, create_data).await;
        assert!(result.is_err(), "Create with empty name should fail");

        // Pre hook should have been called
        assert!(
            CREATE_PRE_CALLED.load(Ordering::SeqCst),
            "create::one::pre hook should have been called"
        );
        // Post hook should NOT have been called (validation failed)
        assert!(
            !CREATE_POST_CALLED.load(Ordering::SeqCst),
            "create::one::post hook should NOT be called when validation fails"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_read_hooks_called() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create an item
        let create_data = HookTestItemCreate {
            name: "read test".to_string(),
        };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        // Reset flags before read
        reset_hook_flags();

        // Read the item
        let result = HookTestItem::get_one(&db, created.id).await;
        assert!(result.is_ok(), "Get one should succeed: {:?}", result);

        // Verify read hooks were called
        assert!(
            READ_PRE_CALLED.load(Ordering::SeqCst),
            "read::one::pre hook should have been called"
        );
        assert!(
            READ_POST_CALLED.load(Ordering::SeqCst),
            "read::one::post hook should have been called"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_update_hooks_called() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create an item
        let create_data = HookTestItemCreate {
            name: "update test".to_string(),
        };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        // Reset flags before update
        reset_hook_flags();

        // Update the item
        let update_data = HookTestItemUpdate {
            name: Some(Some("updated name".to_string())),
        };
        let result = HookTestItem::update(&db, created.id, update_data).await;
        assert!(result.is_ok(), "Update should succeed: {:?}", result);

        // Verify update hooks were called
        assert!(
            UPDATE_PRE_CALLED.load(Ordering::SeqCst),
            "update::one::pre hook should have been called"
        );
        assert!(
            UPDATE_POST_CALLED.load(Ordering::SeqCst),
            "update::one::post hook should have been called"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_update_pre_hook_validation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create an item
        let create_data = HookTestItemCreate {
            name: "validation test".to_string(),
        };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        // Reset flags before update
        reset_hook_flags();

        // Try to update with empty name - should fail validation in pre hook
        let update_data = HookTestItemUpdate {
            name: Some(Some("".to_string())),
        };
        let result = HookTestItem::update(&db, created.id, update_data).await;
        assert!(result.is_err(), "Update with empty name should fail");

        // Pre hook should have been called
        assert!(
            UPDATE_PRE_CALLED.load(Ordering::SeqCst),
            "update::one::pre hook should have been called"
        );
        // Post hook should NOT have been called (validation failed)
        assert!(
            !UPDATE_POST_CALLED.load(Ordering::SeqCst),
            "update::one::post hook should NOT be called when validation fails"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_hooks_called() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create an item
        let create_data = HookTestItemCreate {
            name: "delete test".to_string(),
        };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        // Reset flags before delete
        reset_hook_flags();

        // Delete the item
        let result = HookTestItem::delete(&db, created.id).await;
        assert!(result.is_ok(), "Delete should succeed: {:?}", result);

        // Verify delete hooks were called
        assert!(
            DELETE_PRE_CALLED.load(Ordering::SeqCst),
            "delete::one::pre hook should have been called"
        );
        assert!(
            DELETE_POST_CALLED.load(Ordering::SeqCst),
            "delete::one::post hook should have been called"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_hook_execution_order() {
        // This test verifies that hooks are executed in the correct order:
        // pre -> body (default) -> post
        //
        // We can verify this by checking that if pre fails, post is never called.
        // The test_create_pre_hook_validation and test_update_pre_hook_validation
        // tests already verify this behavior.
        //
        // Additionally, we verify that all hooks in a successful operation are called.
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Perform a full CRUD cycle
        let create_data = HookTestItemCreate {
            name: "order test".to_string(),
        };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        // Check create hooks
        assert!(CREATE_PRE_CALLED.load(Ordering::SeqCst));
        assert!(CREATE_POST_CALLED.load(Ordering::SeqCst));

        reset_hook_flags();
        let _ = HookTestItem::get_one(&db, created.id).await;
        assert!(READ_PRE_CALLED.load(Ordering::SeqCst));
        assert!(READ_POST_CALLED.load(Ordering::SeqCst));

        reset_hook_flags();
        let update_data = HookTestItemUpdate {
            name: Some(Some("updated".to_string())),
        };
        let _ = HookTestItem::update(&db, created.id, update_data).await;
        assert!(UPDATE_PRE_CALLED.load(Ordering::SeqCst));
        assert!(UPDATE_POST_CALLED.load(Ordering::SeqCst));

        reset_hook_flags();
        let _ = HookTestItem::delete(&db, created.id).await;
        assert!(DELETE_PRE_CALLED.load(Ordering::SeqCst));
        assert!(DELETE_POST_CALLED.load(Ordering::SeqCst));
    }

    #[tokio::test]
    #[serial]
    async fn test_create_many_batch_operation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create multiple items in a batch
        let items = vec![
            HookTestItemCreate { name: "item1".to_string() },
            HookTestItemCreate { name: "item2".to_string() },
            HookTestItemCreate { name: "item3".to_string() },
        ];

        let result = HookTestItem::create_many(&db, items).await;
        assert!(result.is_ok(), "create_many should succeed: {:?}", result);

        let created = result.unwrap();
        assert_eq!(created.len(), 3, "Should create 3 items");
        assert_eq!(created[0].name, "item1");
        assert_eq!(created[1].name, "item2");
        assert_eq!(created[2].name, "item3");
    }

    #[tokio::test]
    #[serial]
    async fn test_update_many_batch_operation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create items
        let items = vec![
            HookTestItemCreate { name: "original1".to_string() },
            HookTestItemCreate { name: "original2".to_string() },
        ];
        let created = HookTestItem::create_many(&db, items)
            .await
            .expect("create_many should succeed");

        // Update multiple items
        let updates: Vec<(Uuid, HookTestItemUpdate)> = created
            .iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    item.id,
                    HookTestItemUpdate {
                        name: Some(Some(format!("updated{}", i + 1))),
                    },
                )
            })
            .collect();

        let result = HookTestItem::update_many(&db, updates).await;
        assert!(result.is_ok(), "update_many should succeed: {:?}", result);

        let updated = result.unwrap();
        assert_eq!(updated.len(), 2, "Should update 2 items");
        assert_eq!(updated[0].name, "updated1");
        assert_eq!(updated[1].name, "updated2");
    }

    #[tokio::test]
    #[serial]
    async fn test_batch_size_limits() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Try to create more than 100 items (should fail due to security limit)
        let items: Vec<HookTestItemCreate> = (0..101)
            .map(|i| HookTestItemCreate {
                name: format!("item{}", i),
            })
            .collect();

        let result = HookTestItem::create_many(&db, items).await;
        assert!(result.is_err(), "create_many with 101 items should fail");

        // Verify it's a bad_request error by checking the error message contains "limited"
        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("limited") || err_msg.contains("100"),
            "Error message should mention batch limit: {}",
            err_msg
        );
    }

    // ========================================================================
    // BATCH OPERATION EDGE CASE TESTS
    // ========================================================================

    #[tokio::test]
    #[serial]
    async fn test_create_many_empty_batch() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Empty batch should succeed with empty result
        let items: Vec<HookTestItemCreate> = vec![];
        let result = HookTestItem::create_many(&db, items).await;
        assert!(result.is_ok(), "create_many with empty batch should succeed");
        assert_eq!(result.unwrap().len(), 0, "Empty batch should return empty result");
    }

    #[tokio::test]
    #[serial]
    async fn test_update_many_empty_batch() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Empty batch should succeed with empty result
        let updates: Vec<(Uuid, HookTestItemUpdate)> = vec![];
        let result = HookTestItem::update_many(&db, updates).await;
        assert!(result.is_ok(), "update_many with empty batch should succeed");
        assert_eq!(result.unwrap().len(), 0, "Empty batch should return empty result");
    }

    #[tokio::test]
    #[serial]
    async fn test_update_many_batch_size_limit() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Try to update more than 100 items (should fail due to security limit)
        let updates: Vec<(Uuid, HookTestItemUpdate)> = (0..101)
            .map(|_| (Uuid::new_v4(), HookTestItemUpdate { name: Some(Some("test".to_string())) }))
            .collect();

        let result = HookTestItem::update_many(&db, updates).await;
        assert!(result.is_err(), "update_many with 101 items should fail");

        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("limited") || err_msg.contains("100"),
            "Error message should mention batch limit: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_update_many_nonexistent_id() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Try to update a non-existent item
        let updates = vec![(
            Uuid::new_v4(), // Random ID that doesn't exist
            HookTestItemUpdate { name: Some(Some("updated".to_string())) },
        )];

        let result = HookTestItem::update_many(&db, updates).await;
        assert!(result.is_err(), "update_many with non-existent ID should fail");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_many_at_limit() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create exactly 100 items (should succeed - at the limit)
        let items: Vec<HookTestItemCreate> = (0..100)
            .map(|i| HookTestItemCreate {
                name: format!("item{}", i),
            })
            .collect();

        let result = HookTestItem::create_many(&db, items).await;
        assert!(result.is_ok(), "create_many with exactly 100 items should succeed");
        assert_eq!(result.unwrap().len(), 100, "Should create exactly 100 items");
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_many_batch_operation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create items first
        let items = vec![
            HookTestItemCreate { name: "delete1".to_string() },
            HookTestItemCreate { name: "delete2".to_string() },
            HookTestItemCreate { name: "delete3".to_string() },
        ];
        let created = HookTestItem::create_many(&db, items)
            .await
            .expect("create_many should succeed");

        let ids: Vec<Uuid> = created.iter().map(|item| item.id).collect();

        // Delete all items in batch
        let result = HookTestItem::delete_many(&db, ids.clone()).await;
        assert!(result.is_ok(), "delete_many should succeed");

        let deleted_ids = result.unwrap();
        assert_eq!(deleted_ids.len(), 3, "Should delete 3 items");

        // Verify items are actually deleted
        for id in ids {
            let fetch_result = HookTestItem::get_one(&db, id).await;
            assert!(fetch_result.is_err(), "Deleted item should not be found");
        }
    }

    // ========================================================================
    // HOOK FAILURE SCENARIO TESTS
    // ========================================================================

    #[tokio::test]
    #[serial]
    async fn test_pre_hook_failure_prevents_operation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create with empty name - pre hook validation should fail
        let create_data = HookTestItemCreate { name: "".to_string() };
        let result = HookTestItem::create(&db, create_data).await;

        assert!(result.is_err(), "Create should fail due to pre-hook validation");
        assert!(CREATE_PRE_CALLED.load(Ordering::SeqCst), "Pre-hook should be called");
        assert!(!CREATE_POST_CALLED.load(Ordering::SeqCst), "Post-hook should NOT be called after pre-hook failure");
    }

    #[tokio::test]
    #[serial]
    async fn test_update_pre_hook_failure_prevents_operation() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // First create a valid item
        let create_data = HookTestItemCreate { name: "valid".to_string() };
        let created = HookTestItem::create(&db, create_data)
            .await
            .expect("Create should succeed");

        reset_hook_flags();

        // Update with empty name - pre hook validation should fail
        let update_data = HookTestItemUpdate { name: Some(Some("".to_string())) };
        let result = HookTestItem::update(&db, created.id, update_data).await;

        assert!(result.is_err(), "Update should fail due to pre-hook validation");
        assert!(UPDATE_PRE_CALLED.load(Ordering::SeqCst), "Pre-hook should be called");
        assert!(!UPDATE_POST_CALLED.load(Ordering::SeqCst), "Post-hook should NOT be called after pre-hook failure");

        // Verify item was not modified
        let fetched = HookTestItem::get_one(&db, created.id).await.expect("Should fetch item");
        assert_eq!(fetched.name, "valid", "Item should not be modified after failed update");
    }

    #[tokio::test]
    #[serial]
    async fn test_read_nonexistent_returns_not_found() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        let result = HookTestItem::get_one(&db, Uuid::new_v4()).await;
        assert!(result.is_err(), "Reading non-existent item should fail");

        // Pre-hook should still be called
        assert!(READ_PRE_CALLED.load(Ordering::SeqCst), "read pre-hook should be called even for not found");
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_nonexistent_returns_not_found() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        let result = HookTestItem::delete(&db, Uuid::new_v4()).await;
        assert!(result.is_err(), "Deleting non-existent item should fail");

        // Pre-hook should still be called
        assert!(DELETE_PRE_CALLED.load(Ordering::SeqCst), "delete pre-hook should be called even for not found");
    }

    // ========================================================================
    // CRUDRESOURCE TRAIT METHOD TESTS
    // ========================================================================

    #[tokio::test]
    #[serial]
    async fn test_get_all_returns_list() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create multiple items
        let items = vec![
            HookTestItemCreate { name: "alpha".to_string() },
            HookTestItemCreate { name: "beta".to_string() },
            HookTestItemCreate { name: "gamma".to_string() },
        ];
        let _ = HookTestItem::create_many(&db, items)
            .await
            .expect("create_many should succeed");

        // Get all with no filter
        let condition = sea_orm::Condition::all();
        let result = HookTestItem::get_all(
            &db,
            &condition,
            <HookTestItem as CRUDResource>::ID_COLUMN,
            sea_orm::Order::Asc,
            0,
            100,
        )
        .await;

        assert!(result.is_ok(), "get_all should succeed");
        let items = result.unwrap();
        assert_eq!(items.len(), 3, "Should return 3 items");
    }

    #[tokio::test]
    #[serial]
    async fn test_get_all_with_pagination() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create 5 items
        let items: Vec<HookTestItemCreate> = (0..5)
            .map(|i| HookTestItemCreate { name: format!("item{}", i) })
            .collect();
        let _ = HookTestItem::create_many(&db, items)
            .await
            .expect("create_many should succeed");

        // Get first page (3 items)
        let condition = sea_orm::Condition::all();
        let result = HookTestItem::get_all(
            &db,
            &condition,
            <HookTestItem as CRUDResource>::ID_COLUMN,
            sea_orm::Order::Asc,
            0,
            3,
        )
        .await;

        assert!(result.is_ok(), "get_all should succeed");
        assert_eq!(result.unwrap().len(), 3, "Should return 3 items on first page");

        // Get second page (2 items)
        let result = HookTestItem::get_all(
            &db,
            &condition,
            <HookTestItem as CRUDResource>::ID_COLUMN,
            sea_orm::Order::Asc,
            3,
            3,
        )
        .await;

        assert!(result.is_ok(), "get_all should succeed");
        assert_eq!(result.unwrap().len(), 2, "Should return 2 items on second page");
    }

    #[tokio::test]
    #[serial]
    async fn test_total_count() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Create 4 items
        let items: Vec<HookTestItemCreate> = (0..4)
            .map(|i| HookTestItemCreate { name: format!("count_item{}", i) })
            .collect();
        let _ = HookTestItem::create_many(&db, items)
            .await
            .expect("create_many should succeed");

        // Get total count
        let condition = sea_orm::Condition::all();
        let count = HookTestItem::total_count(&db, &condition).await;

        assert_eq!(count, 4, "Total count should be 4");
    }

    #[tokio::test]
    #[serial]
    async fn test_total_count_empty() {
        reset_hook_flags();
        let db = setup_db().await.expect("Failed to setup database");

        // Empty table
        let condition = sea_orm::Condition::all();
        let count = HookTestItem::total_count(&db, &condition).await;

        assert_eq!(count, 0, "Total count should be 0 for empty table");
    }

    #[test]
    fn test_sortable_columns() {
        let columns = HookTestItem::sortable_columns();
        assert!(!columns.is_empty(), "Should have sortable columns");

        // Check that expected columns are present (name is marked sortable in entity)
        let column_names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
        assert!(column_names.contains(&"name"), "name should be sortable");
    }

    #[test]
    fn test_filterable_columns() {
        let columns = HookTestItem::filterable_columns();
        assert!(!columns.is_empty(), "Should have filterable columns");

        // Check that expected columns are present (name is marked filterable in entity)
        let column_names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
        assert!(column_names.contains(&"name"), "name should be filterable");
    }

    #[test]
    fn test_default_index_column() {
        // Should return ID column by default
        let column = HookTestItem::default_index_column();
        // Just verify it doesn't panic - column comparison would need more setup
        let _ = format!("{:?}", column);
    }

    #[test]
    fn test_like_filterable_columns() {
        // Default implementation returns empty vec
        let columns = HookTestItem::like_filterable_columns();
        // May be empty or have values depending on entity configuration
        let _ = columns; // Just verify it doesn't panic
    }

    #[test]
    fn test_fulltext_searchable_columns() {
        // Default implementation returns empty vec
        let columns = HookTestItem::fulltext_searchable_columns();
        // May be empty or have values depending on entity configuration
        let _ = columns; // Just verify it doesn't panic
    }

    #[test]
    fn test_is_enum_field() {
        // Default implementation returns false
        assert!(!HookTestItem::is_enum_field("name"), "name should not be an enum field");
        assert!(!HookTestItem::is_enum_field("nonexistent"), "nonexistent should not be an enum field");
    }

    #[test]
    fn test_normalize_enum_value() {
        // Default implementation returns None
        assert!(HookTestItem::normalize_enum_value("name", "test").is_none());
        assert!(HookTestItem::normalize_enum_value("nonexistent", "value").is_none());
    }

    #[test]
    fn test_resource_constants() {
        // Verify all trait constants are set correctly
        assert_eq!(HookTestItem::RESOURCE_NAME_SINGULAR, "hook_test_item");
        assert_eq!(HookTestItem::RESOURCE_NAME_PLURAL, "hook_test_items");
        assert_eq!(HookTestItem::TABLE_NAME, "hook_test_items");
        // Description is auto-generated when not specified
        assert!(!HookTestItem::RESOURCE_DESCRIPTION.is_empty(), "Description should be set");
        // Fulltext language defaults to "english"
        assert_eq!(HookTestItem::FULLTEXT_LANGUAGE, "english");
    }
}
