//! Coverage for the `get_all` (LIST) BATCH join-loading path at depth > 1.
//!
//! The `get_all` batch loader in
//! `crudcrate-derive/src/codegen/joins/loading.rs` has two distinct branches
//! for `Vec<Child>` join fields:
//!   - depth == 1: a single `WHERE fk IN (...)` query grouped in memory.
//!   - depth  > 1: the same batch query for the immediate children, then a
//!     per-child recursive `Child::get_one(...)` so grandchildren are loaded.
//!
//! This file exercises the depth > 1 branch end-to-end through a 3-level
//! hierarchy: Org -> Team (`org_id` FK) -> Member (`team_id` FK). `Org.teams`
//! uses `join(one, all, depth = 3)`, so a list of orgs must populate each
//! org's teams AND each team's members (grandchildren). The `get_one` path is
//! contrasted against the same fixture.

use axum::http::StatusCode;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use test_suite::http;
use uuid::Uuid;

pub mod org {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jgd_orgs")]
    #[crudcrate(generate_router, api_struct = "JgdOrg", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 3))]
        pub teams: Vec<super::team::JgdTeam>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::team::Entity")]
        Teams,
    }

    impl Related<super::team::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Teams.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod team {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jgd_teams")]
    #[crudcrate(generate_router, api_struct = "JgdTeam", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub org_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 2))]
        pub members: Vec<super::member::JgdMember>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::org::Entity",
            from = "Column::OrgId",
            to = "super::org::Column::Id"
        )]
        Org,
        #[sea_orm(has_many = "super::member::Entity")]
        Members,
    }

    impl Related<super::org::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Org.def()
        }
    }

    impl Related<super::member::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Members.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod member {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jgd_members")]
    #[crudcrate(generate_router, api_struct = "JgdMember", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub team_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
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

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(org::Entity, team::Entity, member::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/orgs", org::JgdOrg::router(db).into())
        .nest("/teams", team::JgdTeam::router(db).into())
        .nest("/members", member::JgdMember::router(db).into())
}

/// Seed one org with `n_teams` teams, each team with `members_per_team`
/// members. Returns the new org's id. Member names embed their team name so
/// tests can assert parent/child identity regardless of row ordering.
async fn seed_hierarchy(
    db: &DatabaseConnection,
    org_name: &str,
    n_teams: usize,
    members_per_team: usize,
) -> Uuid {
    let created_org = org::JgdOrg::create(
        db,
        org::JgdOrgCreate {
            name: org_name.to_string(),
        },
    )
    .await
    .expect("create org");
    let org_id = created_org.id;

    for t in 0..n_teams {
        let created_team = team::JgdTeam::create(
            db,
            team::JgdTeamCreate {
                org_id,
                name: format!("{org_name}-team-{t}"),
            },
        )
        .await
        .expect("create team");

        for m in 0..members_per_team {
            member::JgdMember::create(
                db,
                member::JgdMemberCreate {
                    team_id: created_team.id,
                    name: format!("{org_name}-team-{t}-member-{m}"),
                },
            )
            .await
            .expect("create member");
        }
    }

    org_id
}

/// LIST endpoint exercises the batch `get_all` loader. `Org.teams` is
/// `join(one, all, depth = 3)` so the depth > 1 branch must recurse into each
/// team via `JgdTeam::get_one` and populate the grandchild `members`.
#[tokio::test]
async fn list_orgs_populates_teams_and_grandchild_members() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "acme", 2, 2).await;

    let app = app(&db);
    let (status, body) = http::get(&app, "/orgs").await;
    assert_eq!(status, StatusCode::OK);

    let orgs = body.as_array().expect("list response is an array");
    let org_row = orgs
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_id.to_string()))
        .expect("seeded org present in list");

    let teams = org_row["teams"]
        .as_array()
        .expect("teams populated on list row");
    assert_eq!(teams.len(), 2, "org should have 2 teams in get_all");

    for team in teams {
        let members = team["members"]
            .as_array()
            .expect("grandchild members populated on each team in get_all");
        assert_eq!(
            members.len(),
            2,
            "each team should carry its 2 members through the batch depth>1 path"
        );
    }
}

/// Same fixture through `get_one` for contrast: the single-item join loader
/// must produce the identical nested shape.
#[tokio::test]
async fn get_one_org_populates_teams_and_grandchild_members() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "globex", 2, 2).await;

    let app = app(&db);
    let (status, org) = http::get(&app, &format!("/orgs/{org_id}")).await;
    assert_eq!(status, StatusCode::OK);

    let teams = org["teams"].as_array().expect("teams populated on get_one");
    assert_eq!(teams.len(), 2);

    for team in teams {
        let members = team["members"]
            .as_array()
            .expect("grandchild members populated on get_one");
        assert_eq!(members.len(), 2);
    }
}

/// `get_all` and `get_one` must agree on the nested member shape for the same
/// data. The two loaders share no code, so this guards drift between them.
#[tokio::test]
async fn list_and_get_one_agree_on_nested_shape() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "initech", 3, 1).await;

    let app = app(&db);
    let (_, list_body) = http::get(&app, "/orgs").await;
    let (_, one_body) = http::get(&app, &format!("/orgs/{org_id}")).await;

    let from_list = list_body
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_id.to_string()))
        .expect("org in list");

    let list_member_total: usize = from_list["teams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["members"].as_array().unwrap().len())
        .sum();
    let one_member_total: usize = one_body["teams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["members"].as_array().unwrap().len())
        .sum();

    assert_eq!(from_list["teams"].as_array().unwrap().len(), 3);
    assert_eq!(one_body["teams"].as_array().unwrap().len(), 3);
    assert_eq!(
        list_member_total, one_member_total,
        "get_all and get_one must load the same grandchild count"
    );
    assert_eq!(list_member_total, 3, "3 teams x 1 member each");
}

/// Multiple parents in one list call: the batch loader collects every org's
/// id up front and issues one team query, then recurses per team. Every org
/// row must be independently and correctly populated.
#[tokio::test]
async fn list_multiple_orgs_each_gets_own_subtree() {
    let db = setup_test_db().await.expect("db setup");
    let org_a = seed_hierarchy(&db, "alpha", 1, 3).await;
    let org_b = seed_hierarchy(&db, "bravo", 2, 1).await;

    let app = app(&db);
    let (status, body) = http::get(&app, "/orgs").await;
    assert_eq!(status, StatusCode::OK);

    let orgs = body.as_array().unwrap();
    assert_eq!(orgs.len(), 2, "both seeded orgs returned");

    let a = orgs
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_a.to_string()))
        .unwrap();
    let b = orgs
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_b.to_string()))
        .unwrap();

    let a_teams = a["teams"].as_array().unwrap();
    assert_eq!(a_teams.len(), 1, "alpha has 1 team");
    assert_eq!(
        a_teams[0]["members"].as_array().unwrap().len(),
        3,
        "alpha's single team has 3 members"
    );

    let b_teams = b["teams"].as_array().unwrap();
    assert_eq!(b_teams.len(), 2, "bravo has 2 teams");
    for team in b_teams {
        assert_eq!(
            team["members"].as_array().unwrap().len(),
            1,
            "each bravo team has 1 member"
        );
    }
}

/// An org with teams that have no members: the depth > 1 recursion must still
/// run for each team and yield an empty members array, not null/missing.
#[tokio::test]
async fn list_org_with_childless_teams_yields_empty_member_arrays() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "hooli", 2, 0).await;

    let app = app(&db);
    let (status, body) = http::get(&app, "/orgs").await;
    assert_eq!(status, StatusCode::OK);

    let org_row = body
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_id.to_string()))
        .unwrap();

    let teams = org_row["teams"].as_array().unwrap();
    assert_eq!(teams.len(), 2);
    for team in teams {
        let members = team["members"]
            .as_array()
            .expect("members is an array even when empty");
        assert!(members.is_empty(), "childless team has empty members array");
    }
}

/// An org with no teams: the batch loader must produce an empty teams array.
#[tokio::test]
async fn list_org_with_no_teams_yields_empty_teams_array() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "umbrella", 0, 0).await;

    let app = app(&db);
    let (status, body) = http::get(&app, "/orgs").await;
    assert_eq!(status, StatusCode::OK);

    let org_row = body
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_id.to_string()))
        .unwrap();

    let teams = org_row["teams"].as_array().expect("teams is an array");
    assert!(teams.is_empty(), "org with no teams has empty teams array");
}

/// Direct trait call rather than HTTP: `JgdOrg::get_all` returns the api
/// struct (`JgdOrg`) whose typed `teams` and nested `members` fields are
/// populated. Verifies the generated struct fields, not just JSON.
#[tokio::test]
async fn get_all_trait_call_populates_typed_nested_fields() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "stark", 2, 2).await;

    let condition = sea_orm::Condition::all();
    let orgs = org::JgdOrg::get_all(
        &db,
        &condition,
        org::Column::Name,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("get_all");

    let org = orgs
        .iter()
        .find(|o| o.id == org_id)
        .expect("seeded org present");
    assert_eq!(org.teams.len(), 2, "typed teams field populated");
    for team in &org.teams {
        assert_eq!(team.members.len(), 2, "typed grandchild members populated");
        assert_eq!(team.org_id, org_id, "child FK back-reference is correct");
    }
}

/// The grandchild names must round-trip exactly through the batch path,
/// confirming the recursion attaches the right members to the right team
/// (not just the right counts).
#[tokio::test]
async fn list_preserves_grandchild_identity_per_team() {
    let db = setup_test_db().await.expect("db setup");
    let org_id = seed_hierarchy(&db, "wayne", 2, 2).await;

    let app = app(&db);
    let (_, body) = http::get(&app, "/orgs").await;

    let org_row = body
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_str() == Some(&org_id.to_string()))
        .unwrap();

    for team in org_row["teams"].as_array().unwrap() {
        let team_name = team["name"].as_str().unwrap();
        for member in team["members"].as_array().unwrap() {
            let member_name = member["name"].as_str().unwrap();
            assert!(
                member_name.starts_with(team_name),
                "member '{member_name}' should belong under team '{team_name}'"
            );
        }
    }
}
