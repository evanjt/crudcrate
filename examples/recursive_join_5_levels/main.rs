#![allow(clippy::needless_for_each)]

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set, entity::prelude::*,
};
use std::time::Duration;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

// Import local models
mod models;
use models::{branch, company, department, employee, project, task};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CrudCrate 5-Deep Join API (6 Levels)",
        description = "Demonstrates TRUE 5 JOIN operations across 6 levels: Company → Branch → Department → Employee → Project → Task. All relationships are loaded automatically with a single API call, showcasing CrudCrate's depth=5 capability.",
        version = "1.0.0",
        contact(
            name = "CrudCrate Documentation",
            url = "https://github.com/evanjt/crudcrate"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "Development server")
    ),
    tags(
        (name = "companies", description = "Company management (Level 1 - loads all 6 levels / 5 joins)"),
        (name = "branches", description = "Branch management (Level 2 - loads 5 levels down)"),
        (name = "departments", description = "Department management (Level 3 - loads 4 levels down)"),
        (name = "employees", description = "Employee management (Level 4 - loads 3 levels down)"),
        (name = "projects", description = "Project management (Level 5 - loads 2 levels down)"),
        (name = "tasks", description = "Task management (Level 6 - leaf node)")
    )
)]
struct ApiDoc;

async fn setup_database() -> DatabaseConnection {
    let mut opt = ConnectOptions::new("sqlite::memory:".to_owned());
    opt.max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(30))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(3600))
        .sqlx_logging(false);

    let db = Database::connect(opt).await.unwrap();

    create_tables(&db).await;
    seed_data(&db).await;

    db
}

async fn create_tables(db: &DatabaseConnection) {
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    println!("Creating companies table...");
    let stmt = schema.create_table_from_entity(company::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ companies table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }

    println!("Creating branches table...");
    let stmt = schema.create_table_from_entity(branch::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ branches table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }

    println!("Creating departments table...");
    let stmt = schema.create_table_from_entity(department::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ departments table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }

    println!("Creating employees table...");
    let stmt = schema.create_table_from_entity(employee::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ employees table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }

    println!("Creating projects table...");
    let stmt = schema.create_table_from_entity(project::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ projects table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }

    println!("Creating tasks table...");
    let stmt = schema.create_table_from_entity(task::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ tasks table created"),
        Err(e) => println!("❌ Failed: {e:?}"),
    }
}

async fn seed_data(db: &DatabaseConnection) {
    println!("\n🌱 Seeding 5-level deep organizational data...");

    // Level 1: Company
    let company_id = Uuid::new_v4();
    company::ActiveModel {
        id: Set(company_id),
        name: Set("TechCorp Global".to_string()),
        industry: Set("Technology".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // Level 2: Branches
    let branch_sf_id = Uuid::new_v4();
    branch::ActiveModel {
        id: Set(branch_sf_id),
        company_id: Set(company_id),
        name: Set("San Francisco HQ".to_string()),
        city: Set("San Francisco".to_string()),
        country: Set("USA".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let branch_ny_id = Uuid::new_v4();
    branch::ActiveModel {
        id: Set(branch_ny_id),
        company_id: Set(company_id),
        name: Set("New York Office".to_string()),
        city: Set("New York".to_string()),
        country: Set("USA".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // Level 3: Departments
    let dept_eng_id = Uuid::new_v4();
    department::ActiveModel {
        id: Set(dept_eng_id),
        branch_id: Set(branch_sf_id),
        name: Set("Engineering".to_string()),
        code: Set("ENG".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let dept_sales_id = Uuid::new_v4();
    department::ActiveModel {
        id: Set(dept_sales_id),
        branch_id: Set(branch_ny_id),
        name: Set("Sales".to_string()),
        code: Set("SALES".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // Level 4: Employees
    let emp1_id = Uuid::new_v4();
    employee::ActiveModel {
        id: Set(emp1_id),
        department_id: Set(dept_eng_id),
        name: Set("Alice Johnson".to_string()),
        position: Set("Senior Developer".to_string()),
        email: Set("alice@techcorp.com".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let emp2_id = Uuid::new_v4();
    employee::ActiveModel {
        id: Set(emp2_id),
        department_id: Set(dept_eng_id),
        name: Set("Bob Smith".to_string()),
        position: Set("DevOps Engineer".to_string()),
        email: Set("bob@techcorp.com".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let emp3_id = Uuid::new_v4();
    employee::ActiveModel {
        id: Set(emp3_id),
        department_id: Set(dept_sales_id),
        name: Set("Carol Davis".to_string()),
        position: Set("Sales Manager".to_string()),
        email: Set("carol@techcorp.com".to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // Level 5: Projects
    let proj1_id = Uuid::new_v4();
    project::ActiveModel {
        id: Set(proj1_id),
        employee_id: Set(emp1_id),
        name: Set("API Redesign".to_string()),
        status: Set("In Progress".to_string()),
        budget: Set(Some(150000)),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let proj2_id = Uuid::new_v4();
    project::ActiveModel {
        id: Set(proj2_id),
        employee_id: Set(emp1_id),
        name: Set("Mobile App v2".to_string()),
        status: Set("Planning".to_string()),
        budget: Set(Some(200000)),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let proj3_id = Uuid::new_v4();
    project::ActiveModel {
        id: Set(proj3_id),
        employee_id: Set(emp2_id),
        name: Set("Infrastructure Upgrade".to_string()),
        status: Set("In Progress".to_string()),
        budget: Set(Some(100000)),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    let proj4_id = Uuid::new_v4();
    project::ActiveModel {
        id: Set(proj4_id),
        employee_id: Set(emp3_id),
        name: Set("Q4 Sales Campaign".to_string()),
        status: Set("Active".to_string()),
        budget: Set(Some(50000)),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // Level 6: Tasks (completing the 5th JOIN!)
    task::ActiveModel {
        id: Set(Uuid::new_v4()),
        project_id: Set(proj1_id),
        title: Set("Design new API endpoints".to_string()),
        status: Set("Done".to_string()),
        completed: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    task::ActiveModel {
        id: Set(Uuid::new_v4()),
        project_id: Set(proj1_id),
        title: Set("Implement authentication".to_string()),
        status: Set("In Progress".to_string()),
        completed: Set(false),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    task::ActiveModel {
        id: Set(Uuid::new_v4()),
        project_id: Set(proj2_id),
        title: Set("UI Mockups".to_string()),
        status: Set("Done".to_string()),
        completed: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    task::ActiveModel {
        id: Set(Uuid::new_v4()),
        project_id: Set(proj3_id),
        title: Set("Migrate to Kubernetes".to_string()),
        status: Set("In Progress".to_string()),
        completed: Set(false),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    task::ActiveModel {
        id: Set(Uuid::new_v4()),
        project_id: Set(proj4_id),
        title: Set("Create campaign materials".to_string()),
        status: Set("Done".to_string()),
        completed: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    println!("✅ Seeded complete organizational hierarchy!");
    println!("   1 Company → 2 Branches → 2 Departments → 3 Employees → 4 Projects → 5 Tasks");
    println!("   (6 levels total = 5 JOIN operations)");
}

#[tokio::main]
async fn main() {
    let db = setup_database().await;

    // Build OpenAPI router
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/companies", company::Company::router(&db))
        .nest("/branches", branch::Branch::router(&db))
        .nest("/departments", department::Department::router(&db))
        .nest("/employees", employee::Employee::router(&db))
        .nest("/projects", project::Project::router(&db))
        .nest("/tasks", task::Task::router(&db))
        .split_for_parts();

    // Add Scalar UI and CORS
    let app = router
        .merge(Scalar::with_url("/scalar", api))
        .layer(CorsLayer::permissive());

    println!("\n🚀 Server running on http://localhost:3000");
    println!("📚 OpenAPI Documentation: http://localhost:3000/scalar\n");

    println!("📊 6-Level Structure with 5 JOIN Operations:");
    println!("   Level 1: Company      (depth=5 configured)");
    println!("   Level 2: ↳ Branches   (join #1, depth=4)");
    println!("   Level 3:   ↳ Departments (join #2, depth=3)");
    println!("   Level 4:     ↳ Employees (join #3, depth=2)");
    println!("   Level 5:       ↳ Projects (join #4, depth=1)");
    println!("   Level 6:         ↳ Tasks (join #5, leaf node)\n");

    println!("🧪 Test the full 5-JOIN deep query:");
    println!("   curl -s http://localhost:3000/companies | jq .");
    println!("   # This single API call performs 5 JOINS across 6 levels automatically!\n");

    println!("🔍 Explore individual levels:");
    println!("   curl -s http://localhost:3000/branches | jq .      # 5 joins, 5 levels deep");
    println!("   curl -s http://localhost:3000/departments | jq .   # 4 joins, 4 levels deep");
    println!("   curl -s http://localhost:3000/employees | jq .     # 3 joins, 3 levels deep");
    println!("   curl -s http://localhost:3000/projects | jq .      # 2 joins, 2 levels deep");
    println!("   curl -s http://localhost:3000/tasks | jq .         # Leaf node, 0 joins\n");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
