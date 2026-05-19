//! Joined Filter Example — filter parents by child-entity columns
//!
//! Demonstrates dot-notation joined filters: `?filter={"vehicles.make":"BMW"}`
//! on a `Customer` endpoint returns only customers who own a BMW. The
//! default handler resolves each joined filter into a scoped sub-query on
//! the child table (`vehicles`), collects matching parent-FK values, and
//! adds `customers.id IN (...)` to the main query.
//!
//! Run:
//!
//! ```bash
//! cargo run --example joined_filter
//! ```
//!
//! Then try:
//!
//! ```bash
//! # All customers (no filter)
//! curl -s 'http://localhost:3000/customers' | jq .
//!
//! # Only BMW owners
//! curl -s 'http://localhost:3000/customers?filter=%7B%22vehicles.make%22%3A%22BMW%22%7D' | jq .
//!
//! # Owners of cars built in 2020 or later
//! curl -s 'http://localhost:3000/customers?filter=%7B%22vehicles.year_gte%22%3A2020%7D' | jq .
//!
//! # Intersection: Alice AND owns a BMW
//! curl -s 'http://localhost:3000/customers?filter=%7B%22name%22%3A%22Alice%22%2C%22vehicles.make%22%3A%22BMW%22%7D' | jq .
//! ```

mod models;

use models::{customer, setup_database, vehicle};
use sea_orm::{ActiveModelTrait, Set};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(info(
    title = "CrudCrate Joined Filter Example",
    description = "Demonstrates dot-notation joined filters.",
    version = "1.0.0"
))]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_database("sqlite::memory:").await?;
    seed(&db).await?;

    let (router, apidocs) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/customers", customer::Customer::router(&db))
        .nest("/vehicles", vehicle::Vehicle::router(&db))
        .split_for_parts();

    let app = router.merge(Scalar::with_url("/docs", apidocs));

    println!("Joined filter example running at http://0.0.0.0:3000");
    println!("Docs: http://0.0.0.0:3000/docs\n");
    println!("Try:");
    println!("  curl 'http://localhost:3000/customers'");
    println!(
        "  curl 'http://localhost:3000/customers?filter=%7B%22vehicles.make%22%3A%22BMW%22%7D'"
    );
    println!(
        "  curl 'http://localhost:3000/customers?filter=%7B%22vehicles.year_gte%22%3A2020%7D'"
    );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn seed(db: &sea_orm::DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    let alice_id = Uuid::new_v4();
    customer::ActiveModel {
        id: Set(alice_id),
        name: Set("Alice".to_string()),
        email: Set("alice@example.com".to_string()),
    }
    .insert(db)
    .await?;

    let bob_id = Uuid::new_v4();
    customer::ActiveModel {
        id: Set(bob_id),
        name: Set("Bob".to_string()),
        email: Set("bob@example.com".to_string()),
    }
    .insert(db)
    .await?;

    let carol_id = Uuid::new_v4();
    customer::ActiveModel {
        id: Set(carol_id),
        name: Set("Carol".to_string()),
        email: Set("carol@example.com".to_string()),
    }
    .insert(db)
    .await?;

    for (customer_id, make, year) in [
        (alice_id, "BMW", 2023),
        (bob_id, "Toyota", 2020),
        (carol_id, "Honda", 2018),
    ] {
        vehicle::ActiveModel {
            id: Set(Uuid::new_v4()),
            customer_id: Set(customer_id),
            make: Set(make.to_string()),
            year: Set(year),
        }
        .insert(db)
        .await?;
    }

    Ok(())
}
