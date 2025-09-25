// Database Index Analysis Tests
// Tests the index analysis functionality (4/148 lines covered in index_analysis.rs)

use crudcrate::CRUDResource;
use common::{setup_test_db, Customer};

mod common;

#[tokio::test]
async fn test_index_analysis() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Test the index analysis functionality
    let result = Customer::analyse_and_display_indexes(&db).await;
    
    // Should complete without error (exercises analyse_indexes_for_resource and display_index_recommendations)
    assert!(result.is_ok(), "Index analysis should complete successfully");

    // Test individual analysis functions
    let recommendations = crudcrate::database::analyse_indexes_for_resource::<Customer>(&db).await;
    assert!(recommendations.is_ok(), "Should generate index recommendations");

    // Test analyser registration (exercises register_analyser function)
    crudcrate::database::register_analyser::<Customer>();
    
    // Test bulk analysis (exercises analyse_all_registered_models)
    let bulk_analysis = crudcrate::database::analyse_all_registered_models(&db, false).await;
    assert!(bulk_analysis.is_ok(), "Bulk analysis should work");

    // Test analyser initialization
    crudcrate::database::ensure_all_analysers_registered();
}