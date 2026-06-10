//! End-to-end HTTP tests for dot-notation joined sorting.
//!
//! `GET /teams?sort=["players.score","ASC"]` orders parent teams by a column on
//! a joined child entity. The handler resolves a `SortConfig::Joined` via
//! `CRUDResource::get_all_joined_sorted`, which the derive macro overrides to
//! order parents by a correlated sub-query over the child column
//! (`SELECT MIN(player.score) FROM js_player WHERE player.team_id = team.id`).
//!
//! These tests confirm the parent order is the real child-driven order (not the
//! parent PK order it silently used before), that plain parent-column sorts are
//! unaffected, and that a non-whitelisted dot-path does not 500.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

pub mod player {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "js_player")]
    #[crudcrate(generate_router, api_struct = "JsPlayer", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub team_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable, sortable)]
        pub score: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::team::Entity",
            from = "Column::TeamId",
            to = "super::team::Column::Id"
        )]
        Team,
    }

    impl Related<super::team::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Team.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod team {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "js_team")]
    #[crudcrate(generate_router, api_struct = "JsTeam", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr = true,
            exclude(create, update),
            join(one, all, depth = 1, sortable("score"))
        )]
        pub players: Vec<super::player::JsPlayer>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::player::Entity")]
        Players,
    }

    impl Related<super::player::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Players.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drops are harmless no-ops.
    // Drop children before parents: create_table_from_entity emits FK constraints
    // from belongs_to relations (player references team).
    for stmt in [
        Table::drop().table(player::Entity).if_exists().to_owned(),
        Table::drop().table(team::Entity).if_exists().to_owned(),
    ] {
        db.execute(&stmt).await?;
    }

    db.execute(&schema.create_table_from_entity(team::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(player::Entity))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/teams", team::JsTeam::router(db).into())
        .nest("/players", player::JsPlayer::router(db).into())
}

/// Seed one team and its players. Player scores are passed explicitly so each
/// test can make the cross-team ordering unambiguous.
async fn seed_team(db: &DatabaseConnection, name: &str, scores: &[i32]) -> Uuid {
    let team = team::JsTeam::create(
        db,
        team::JsTeamCreate {
            name: name.to_string(),
        },
    )
    .await
    .expect("create team");

    for (i, score) in scores.iter().enumerate() {
        player::JsPlayer::create(
            db,
            player::JsPlayerCreate {
                team_id: team.id,
                name: format!("{name}-p{i}"),
                score: *score,
            },
        )
        .await
        .expect("create player");
    }

    team.id
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn team_names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

/// Sort URLs use percent-encoded JSON so the `[`/`]`/`"`/`.` survive the query
/// string unchanged.
fn sort_query(sort_json: &str) -> String {
    format!(
        "/teams?sort={}",
        percent_encoding::utf8_percent_encode(sort_json, percent_encoding::NON_ALPHANUMERIC)
    )
}

/// Score ranges make the order unambiguous: A {1,2}, B {10,11}, C {100}.
/// MIN(score) is 1, 10, 100 respectively, so ASC must be A, B, C.
async fn seed_three_teams(db: &DatabaseConnection) {
    seed_team(db, "A", &[1, 2]).await;
    seed_team(db, "B", &[10, 11]).await;
    seed_team(db, "C", &[100]).await;
}

#[tokio::test]
async fn joined_sort_score_asc_orders_teams_by_min_child_score() {
    let db = setup_test_db().await.expect("db setup");
    seed_three_teams(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, &sort_query(r#"["players.score","ASC"]"#)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        team_names(&body),
        vec!["A".to_string(), "B".to_string(), "C".to_string()],
        "ASC must order teams by their smallest player score"
    );
}

#[tokio::test]
async fn joined_sort_score_desc_reverses_team_order() {
    let db = setup_test_db().await.expect("db setup");
    seed_three_teams(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, &sort_query(r#"["players.score","DESC"]"#)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        team_names(&body),
        vec!["C".to_string(), "B".to_string(), "A".to_string()],
        "DESC must reverse the child-score order"
    );
}

/// Joined sort must order by the child column, not the parent PK. Seeding the
/// teams whose name order is the OPPOSITE of their score order guarantees the
/// assertion fails if the handler fell back to the parent default column.
#[tokio::test]
async fn joined_sort_is_not_parent_default_order() {
    let db = setup_test_db().await.expect("db setup");
    seed_team(&db, "zulu", &[1]).await; // lowest score, last alphabetically
    seed_team(&db, "alpha", &[50]).await; // highest score, first alphabetically

    let app = app(&db);
    let (status, body) = get_json(&app, &sort_query(r#"["players.score","ASC"]"#)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        team_names(&body),
        vec!["zulu".to_string(), "alpha".to_string()],
        "child-score ASC puts zulu (1) before alpha (50), opposite to name order"
    );
}

/// A normal parent-column sort is untouched by the joined-sort wiring.
#[tokio::test]
async fn parent_column_sort_still_works() {
    let db = setup_test_db().await.expect("db setup");
    seed_team(&db, "charlie", &[1]).await;
    seed_team(&db, "alpha", &[2]).await;
    seed_team(&db, "bravo", &[3]).await;

    let app = app(&db);
    let (status, body) = get_json(&app, &sort_query(r#"["name","ASC"]"#)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        team_names(&body),
        vec![
            "alpha".to_string(),
            "bravo".to_string(),
            "charlie".to_string()
        ],
        "name ASC orders teams alphabetically regardless of child scores"
    );
}

/// A dot-path that is not declared `sortable(...)` is not a joined sort: it
/// falls back to the parent default order. The request must succeed (no 500).
#[tokio::test]
async fn disallowed_dot_path_does_not_500() {
    let db = setup_test_db().await.expect("db setup");
    seed_three_teams(&db).await;

    let app = app(&db);
    // `players.name` is not in the team's join sortable("score") whitelist.
    let (status, body) = get_json(&app, &sort_query(r#"["players.name","ASC"]"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("list response").len(), 3);

    // An unknown join field is likewise harmless.
    let (status, body) = get_json(&app, &sort_query(r#"["coaches.salary","DESC"]"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("list response").len(), 3);
}

/// A team with no players sorts with a NULL aggregate. The request must not
/// error, and every seeded team must still appear in the result.
#[tokio::test]
async fn team_without_players_is_included_in_joined_sort() {
    let db = setup_test_db().await.expect("db setup");
    seed_team(&db, "withplayers", &[5]).await;
    seed_team(&db, "empty", &[]).await;

    let app = app(&db);
    let (status, body) = get_json(&app, &sort_query(r#"["players.score","ASC"]"#)).await;
    assert_eq!(status, StatusCode::OK);

    let mut names = team_names(&body);
    names.sort();
    assert_eq!(
        names,
        vec!["empty".to_string(), "withplayers".to_string()],
        "childless team is not dropped from the joined-sorted list"
    );
}
