//! A resource declares which route families it mounts.
//!
//! A table derived from a registry or a migration has operations that are not legitimate on it: a
//! row nothing registered would never be reached, and a deleted one returns at the next boot.
//! Refusing such a request in the handler still advertises the operation, so the route is not
//! emitted at all.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use sea_orm::{DatabaseConnection, entity::prelude::*};
use serde_json::json;
use tower::ServiceExt;

mod projection {
    use crudcrate::EntityToModels;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "projected_schedules")]
    #[crudcrate(
        api_struct = "ProjectedSchedule",
        name_singular = "projected_schedule",
        name_plural = "projected_schedules",
        generate_router,
        routes(read, update)
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(sortable, filterable)]
        pub job_name: String,

        pub interval_seconds: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

use projection::ProjectedSchedule;

async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(projection::Entity).await
}

async fn send(app: &Router, method: &str, uri: &str, payload: Option<serde_json::Value>) -> u16 {
    let builder = Request::builder().method(method).uri(uri);
    let request = match payload {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    app.clone()
        .oneshot(request)
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn test_a_declared_family_is_mounted() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/schedules", ProjectedSchedule::router(&db).into());

    assert_eq!(send(&app, "GET", "/schedules", None).await, 200);
}

#[tokio::test]
async fn test_an_undeclared_family_is_not_a_route_at_all() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/schedules", ProjectedSchedule::router(&db).into());

    // 405, not 404: the path is known to the router and the method is not, which is what
    // distinguishes an unmounted operation from a mistyped path.
    assert_eq!(
        send(
            &app,
            "POST",
            "/schedules",
            Some(json!({ "job_name": "janitor", "interval_seconds": 60 })),
        )
        .await,
        405,
        "create is not among the families the resource declares"
    );
    assert_eq!(
        send(&app, "DELETE", "/schedules/batch", Some(json!([]))).await,
        405,
        "neither is delete"
    );
}

#[tokio::test]
async fn test_an_unmounted_operation_is_absent_from_the_document() {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    struct Api;
    let (_, api) = utoipa_axum::router::OpenApiRouter::with_openapi(Api::openapi())
        .nest(
            "/schedules",
            ProjectedSchedule::router(&setup_db().await.expect("db")),
        )
        .split_for_parts();

    let document = serde_json::to_value(&api).expect("the document serialises");
    let paths = &document["paths"];
    assert!(
        paths["/schedules"].get("get").is_some(),
        "a declared family is documented: {paths}"
    );
    assert!(
        paths["/schedules"].get("post").is_none(),
        "an undeclared one is not advertised: {paths}"
    );
    assert!(
        paths["/schedules/{id}"].get("delete").is_none(),
        "nor is its single-row route: {paths}"
    );
}
