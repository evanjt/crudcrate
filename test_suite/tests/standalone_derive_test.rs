// Standalone derive test
// ToCreateModel, ToUpdateModel and ToListModel apply directly to an API struct,
// resolving the active model through the `<Name>ActiveModel` naming fallback
// when no #[active_model] attribute is given.

use crudcrate::traits::MergeIntoActiveModel;
use crudcrate::{ToCreateModel, ToListModel, ToUpdateModel};
use sea_orm::ActiveValue;
use serde_json::json;

mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "standalone_notes")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub type NoteActiveModel = entity::ActiveModel;

#[derive(Clone, Debug, serde::Serialize, ToCreateModel, ToUpdateModel, ToListModel)]
pub struct Note {
    pub id: i32,
    pub title: String,
}

// The derives screen out the dense-format ModelEx companion: this expands to
// nothing, so no ModelExCreate/ModelExUpdate/ModelExList types may exist.
#[derive(Clone, Debug, ToCreateModel, ToUpdateModel, ToListModel)]
pub struct ModelEx {
    pub id: i32,
}

#[test]
fn test_create_model_converts_to_active_model() {
    let create: NoteCreate = serde_json::from_value(json!({"id": 1, "title": "draft"}))
        .expect("create model deserializes");
    let active: NoteActiveModel = create.into();
    assert_eq!(active.title, ActiveValue::Set("draft".to_string()));
}

#[test]
fn test_update_model_merges_set_fields_only() {
    let update: NoteUpdate =
        serde_json::from_value(json!({"title": "revised"})).expect("update model deserializes");
    let existing = NoteActiveModel {
        id: ActiveValue::Unchanged(1),
        title: ActiveValue::Unchanged("draft".to_string()),
    };
    let merged = update
        .merge_into_activemodel(existing)
        .expect("merge succeeds");
    assert_eq!(merged.title, ActiveValue::Set("revised".to_string()));
    assert_eq!(
        merged.id,
        ActiveValue::NotSet,
        "absent field stays untouched"
    );
}

#[test]
fn test_list_model_converts_from_api_struct() {
    let note = Note {
        id: 7,
        title: "draft".to_string(),
    };
    let list: NoteList = note.into();
    let rendered = serde_json::to_value(&list).expect("list model serializes");
    assert_eq!(rendered, json!({"id": 7, "title": "draft"}));
}
