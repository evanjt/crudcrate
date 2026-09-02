use super::*;

// ============================================================================
// Constructor Tests
// ============================================================================

#[test]
fn test_not_found_with_id() {
    let err = ApiError::not_found("User", Some("123".to_string()));
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
    assert_eq!(err.user_message(), "User with ID '123' not found");
}

#[test]
fn test_not_found_without_id() {
    let err = ApiError::not_found("User", None);
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
    assert_eq!(err.user_message(), "User not found");
}

#[test]
fn test_bad_request() {
    let err = ApiError::bad_request("Invalid email format");
    assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
    assert_eq!(err.user_message(), "Invalid email format");
}

#[test]
fn test_unauthorized() {
    let err = ApiError::unauthorized("Invalid credentials");
    assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    assert_eq!(err.user_message(), "Invalid credentials");
}

#[test]
fn test_forbidden() {
    let err = ApiError::forbidden("Insufficient permissions");
    assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    assert_eq!(err.user_message(), "Insufficient permissions");
}

#[test]
fn test_conflict() {
    let err = ApiError::conflict("Email already exists");
    assert_eq!(err.status_code(), StatusCode::CONFLICT);
    assert_eq!(err.user_message(), "Email already exists");
}

#[test]
fn test_validation_failed_single_error() {
    let err = ApiError::validation_failed(vec!["Email is required".to_string()]);
    assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.user_message(), "Email is required");
}

#[test]
fn test_validation_failed_multiple_errors() {
    let err = ApiError::validation_failed(vec![
        "Email is required".to_string(),
        "Password too short".to_string(),
    ]);
    assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        err.user_message(),
        "Validation failed: Email is required, Password too short"
    );
}

#[test]
fn test_database_error() {
    let db_err = DbErr::Type("Type mismatch error".to_string());
    let err = ApiError::database(db_err);
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(err.user_message(), "A database error occurred");
}

#[test]
fn test_internal_error_with_details() {
    let err = ApiError::internal(
        "Processing failed",
        Some("Null pointer exception".to_string()),
    );
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(err.user_message(), "Processing failed");
}

#[test]
fn test_internal_error_without_details() {
    let err = ApiError::internal("Processing failed", None);
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(err.user_message(), "Processing failed");
}

#[test]
fn test_custom_error() {
    let err = ApiError::custom(
        StatusCode::TOO_MANY_REQUESTS,
        "Rate limit exceeded",
        Some("User hit 100 req/min".to_string()),
    );
    assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(err.user_message(), "Rate limit exceeded");
}

// ============================================================================
// DbErr Conversion Tests (Hook Error Patterns)
// ============================================================================

#[test]
fn test_dberr_record_not_found_conversion() {
    let db_err = DbErr::RecordNotFound("User not found".to_string());
    let api_err: ApiError = db_err.into();
    assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
    assert!(api_err.user_message().contains("not found"));
}

#[test]
fn test_dberr_record_not_found_rejects_non_identifier_prefix() {
    // Arbitrary DB-driver text must not be reflected into the response.
    // Driver messages that start with quotes, punctuation, or control chars
    // should fall back to the generic "Resource" label.
    for msg in [
        "'<script>alert(1)</script>' not found",
        "\r\nInjected-Header: evil",
        "\"quoted\" not found",
        "",
        "   ",
        "field-with-dashes not found",
    ] {
        let api_err: ApiError = DbErr::RecordNotFound(msg.to_string()).into();
        assert!(
            api_err.user_message().starts_with("Resource "),
            "msg={msg:?} produced {:?}",
            api_err.user_message()
        );
    }
}

#[test]
fn test_dberr_record_not_found_truncates_overlong_prefix() {
    // A 100-char prefix exceeds the 64-char identifier cap.
    let long_prefix = "A".repeat(100);
    let msg = format!("{long_prefix} not found");
    let api_err: ApiError = DbErr::RecordNotFound(msg).into();
    assert!(api_err.user_message().starts_with("Resource "));
}

#[test]
fn test_dberr_custom_becomes_internal() {
    // All DbErr::Custom variants become 500 Internal Server Error
    let db_err = DbErr::Custom("Something went wrong".to_string());
    let api_err: ApiError = db_err.into();
    assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(api_err.user_message(), "A database error occurred");
}

#[test]
fn test_dberr_type_error() {
    let db_err = DbErr::Type("Type conversion failed".to_string());
    let api_err: ApiError = db_err.into();
    assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(api_err.user_message(), "A database error occurred");
}

#[test]
fn test_dberr_json_error() {
    let db_err = DbErr::Json("JSON parsing failed".to_string());
    let api_err: ApiError = db_err.into();
    assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(api_err.user_message(), "A database error occurred");
}

// ============================================================================
// DbErr Conversion Tests - Simple Behavior
// ============================================================================

#[test]
fn test_dberr_record_not_found_becomes_404() {
    // DbErr::RecordNotFound becomes 404
    let db_err = DbErr::RecordNotFound("Blog post not found".to_string());
    let api_err: ApiError = db_err.into();
    assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
    assert!(api_err.user_message().contains("not found"));
}

#[test]
fn test_all_other_dberr_become_500() {
    // All other DbErr types become 500 Internal Server Error
    let test_cases = vec![
        DbErr::Custom("Any custom error".to_string()),
        DbErr::Type("Type error".to_string()),
        DbErr::Json("JSON error".to_string()),
    ];

    for db_err in test_cases {
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.user_message(), "A database error occurred");
    }
}

// ============================================================================
// Display and Error Trait Tests
// ============================================================================

#[test]
fn test_display_trait() {
    let err = ApiError::bad_request("Test error");
    assert_eq!(format!("{err}"), "Test error");
}

#[test]
fn test_error_trait() {
    let err = ApiError::bad_request("Test error");
    let _: &dyn std::error::Error = &err; // Verify it implements Error trait
}

// ============================================================================
// Status Code Coverage Tests
// ============================================================================

#[test]
fn test_all_status_codes() {
    let test_cases = vec![
        (ApiError::not_found("Test", None), StatusCode::NOT_FOUND),
        (ApiError::bad_request("Test"), StatusCode::BAD_REQUEST),
        (ApiError::unauthorized("Test"), StatusCode::UNAUTHORIZED),
        (ApiError::forbidden("Test"), StatusCode::FORBIDDEN),
        (ApiError::conflict("Test"), StatusCode::CONFLICT),
        (
            ApiError::validation_failed(vec!["Test".to_string()]),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            ApiError::database(DbErr::Conn(sea_orm::RuntimeErr::Internal(
                "Test".to_string(),
            ))),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
        (
            ApiError::internal("Test", None),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
        (
            ApiError::custom(StatusCode::IM_A_TEAPOT, "Test", None),
            StatusCode::IM_A_TEAPOT,
        ),
    ];

    for (err, expected_status) in test_cases {
        assert_eq!(err.status_code(), expected_status);
    }
}
