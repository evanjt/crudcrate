#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "posts")]
#[crudcrate(api_struct = "Post")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub title: String,

    #[crudcrate(filterable)]
    pub content: String,

    // This join should trigger a performance note since it's not cyclic and has no explicit depth
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(all))]
    pub author: Option<Author>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "Author")]
    Author,
}

impl Related<Author> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}