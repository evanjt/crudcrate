mod Model {
    use sea_orm::ActiveValue;
    #[derive(
        Clone,
        Debug,
        serde::Serialize,
        serde::Deserialize,
        crudcrate::ToCreateModel,
        crudcrate::ToUpdateModel,
        utoipa::ToSchema
    )]
    #[active_model = "ActiveModel"]
    pub struct Category {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        pub parent_id: Option<Uuid>,
        #[schema(no_recursion)]
        #[crudcrate(non_db_attr, join(one, all, depth = 1))]
        pub children: Vec<Category>,
    }
    impl From<Model> for Category {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                parent_id: model.parent_id,
                children: vec![],
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Category {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = CategoryCreate;
        type UpdateModel = CategoryUpdate;
        type ListModel = CategoryList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        const RESOURCE_NAME_SINGULAR: &'static str = "categories";
        const RESOURCE_NAME_PLURAL: &'static str = "categories";
        const TABLE_NAME: &'static str = "categories";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages categories items";
        const FULLTEXT_LANGUAGE: &'static str = "english";
        fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("name", Self::ColumnType::Name)]
        }
        fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("name", Self::ColumnType::Name)]
        }
        fn is_enum_field(field_name: &str) -> bool {
            match field_name {
                "id" => {
                    trait __Fallback {
                        const V: bool = false;
                    }
                    impl<T> __Fallback for __Probe<T> {}
                    struct __Probe<T>(::core::marker::PhantomData<T>);
                    #[allow(dead_code)]
                    impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                        const V: bool = true;
                    }
                    <__Probe<Uuid>>::V
                }
                "name" => {
                    trait __Fallback {
                        const V: bool = false;
                    }
                    impl<T> __Fallback for __Probe<T> {}
                    struct __Probe<T>(::core::marker::PhantomData<T>);
                    #[allow(dead_code)]
                    impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                        const V: bool = true;
                    }
                    <__Probe<String>>::V
                }
                "parent_id" => {
                    trait __Fallback {
                        const V: bool = false;
                    }
                    impl<T> __Fallback for __Probe<T> {}
                    struct __Probe<T>(::core::marker::PhantomData<T>);
                    #[allow(dead_code)]
                    impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                        const V: bool = true;
                    }
                    <__Probe<Uuid>>::V
                }
                _ => false,
            }
        }
        fn like_filterable_columns() -> Vec<&'static str> {
            vec!["name"]
        }
        fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![]
        }
        fn scoped_excluded_columns() -> &'static [&'static str] {
            &[]
        }
        fn joined_filterable_columns() -> Vec<crudcrate::JoinedColumnDef> {
            vec![]
        }
        fn joined_sortable_columns() -> Vec<crudcrate::JoinedColumnDef> {
            vec![]
        }
        async fn get_one(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
        ) -> Result<Self, crudcrate::ApiError> {
            use sea_orm::{EntityTrait, ModelTrait, Related};
            let main_model = Box::pin(Self::EntityType::find_by_id(id.clone()).one(db))
                .await?;
            let mut result = match main_model {
                Some(model) => {
                    let loaded_children: Vec<Category> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let query = super::category::Entity::find()
                            .filter(super::category::Column::ParentId.eq(model.id));
                        let related_models = Box::pin(query.all(db)).await?;
                        related_models
                            .into_iter()
                            .map(|m: super::category::Model| Category::from(m))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.children = loaded_children;
                    result
                }
                None => {
                    return Err(
                        crudcrate::ApiError::not_found(
                            Self::RESOURCE_NAME_SINGULAR,
                            Some(id.to_string()),
                        ),
                    );
                }
            };
            Ok(result)
        }
        async fn get_one_scoped(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
            scope: &sea_orm::Condition,
        ) -> Result<Self, crudcrate::ApiError> {
            use sea_orm::{EntityTrait, ModelTrait, Related, QueryFilter};
            let scoped_condition = sea_orm::Condition::all()
                .add(Self::ID_COLUMN.eq(id.clone()))
                .add(scope.clone());
            let main_model = Box::pin(
                    Self::EntityType::find().filter(scoped_condition).one(db),
                )
                .await?;
            let mut result = match main_model {
                Some(model) => {
                    let loaded_children: Vec<Category> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let query = super::category::Entity::find()
                            .filter(super::category::Column::ParentId.eq(model.id));
                        let query = if let Some(child_scope) = <super::category::CategoryList as crudcrate::ScopeFilterable>::scope_condition() {
                            query.filter(child_scope)
                        } else {
                            query
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        related_models
                            .into_iter()
                            .map(|m: super::category::Model| Category::from(m))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.children = loaded_children;
                    result
                }
                None => {
                    return Err(
                        crudcrate::ApiError::not_found(
                            Self::RESOURCE_NAME_SINGULAR,
                            Some(id.to_string()),
                        ),
                    );
                }
            };
            Ok(result)
        }
        async fn get_all(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait, ModelTrait, IdenStatic};
            let mut __query = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction);
            if order_column.as_str() != Self::ID_COLUMN.as_str() {
                __query = __query.order_by(Self::ID_COLUMN, sea_orm::Order::Asc);
            }
            let models = __query.offset(offset).limit(limit).all(db).await?;
            let parent_ids: Vec<crudcrate::PrimaryKeyType<Self>> = models
                .iter()
                .map(|m| m.id.clone())
                .collect();
            let mut children_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<Category>,
            > = Box::pin(async {
                    use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                    let query = super::category::Entity::find()
                        .filter(
                            super::category::Column::ParentId.is_in(parent_ids.clone()),
                        );
                    let all_related = query.all(db).await?;
                    let mut map: std::collections::HashMap<
                        crudcrate::PrimaryKeyType<Self>,
                        Vec<Category>,
                    > = std::collections::HashMap::new();
                    for related_model in all_related {
                        if let Some(parent_id) = related_model.parent_id {
                            map.entry(parent_id)
                                .or_insert_with(Vec::new)
                                .push(Category::from(related_model));
                        }
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.children = children_by_parent
                        .remove(&parent_id)
                        .unwrap_or_default();
                    item
                };
                result.push(Self::ListModel::from(item));
            }
            Ok(result)
        }
        async fn get_all_scoped(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait, ModelTrait, IdenStatic};
            let mut __query = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction);
            if order_column.as_str() != Self::ID_COLUMN.as_str() {
                __query = __query.order_by(Self::ID_COLUMN, sea_orm::Order::Asc);
            }
            let models = __query.offset(offset).limit(limit).all(db).await?;
            let parent_ids: Vec<crudcrate::PrimaryKeyType<Self>> = models
                .iter()
                .map(|m| m.id.clone())
                .collect();
            let mut children_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<Category>,
            > = Box::pin(async {
                    use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                    let query = super::category::Entity::find()
                        .filter(
                            super::category::Column::ParentId.is_in(parent_ids.clone()),
                        );
                    let __child_scope: Option<sea_orm::Condition> = <super::category::CategoryList as crudcrate::ScopeFilterable>::scope_condition();
                    let query = if let Some(ref cs) = __child_scope {
                        query.filter(cs.clone())
                    } else {
                        query
                    };
                    let all_related = query.all(db).await?;
                    let mut map: std::collections::HashMap<
                        crudcrate::PrimaryKeyType<Self>,
                        Vec<Category>,
                    > = std::collections::HashMap::new();
                    for related_model in all_related {
                        if let Some(parent_id) = related_model.parent_id {
                            map.entry(parent_id)
                                .or_insert_with(Vec::new)
                                .push(Category::from(related_model));
                        }
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.children = children_by_parent
                        .remove(&parent_id)
                        .unwrap_or_default();
                    item
                };
                result.push(Self::ListModel::from(item));
            }
            Ok(result)
        }
        async fn create(
            db: &sea_orm::DatabaseConnection,
            data: Self::CreateModel,
        ) -> Result<Self, crudcrate::ApiError> {
            {
                use crudcrate::validation::__auto::ValidatableFallback as _;
                crudcrate::validation::__auto::Probe(&data)
                    .crudcrate_auto_validate()
                    .map_err(crudcrate::ApiError::from)?;
            }
            let active_model: Self::ActiveModelType = data.into();
            let insert_result = Self::EntityType::insert(active_model).exec(db).await?;
            let result = Self::get_one(db, insert_result.last_insert_id.into()).await?;
            Ok(result)
        }
        async fn create_many(
            db: &sea_orm::DatabaseConnection,
            data: Vec<Self::CreateModel>,
        ) -> Result<Vec<Self>, crudcrate::ApiError> {
            {
                use crudcrate::validation::__auto::ValidatableFallback as _;
                for __cc_item in &data {
                    crudcrate::validation::__auto::Probe(__cc_item)
                        .crudcrate_auto_validate()
                        .map_err(crudcrate::ApiError::from)?;
                }
            }
            use sea_orm::{
                ActiveModelTrait, ConnectionTrait, EntityTrait, TransactionTrait,
            };
            if data.is_empty() {
                return Ok(vec![]);
            }
            if data.len() > Self::batch_limit() {
                return Err(
                    crudcrate::ApiError::bad_request(
                        format!(
                            "Batch create limited to {} items. Received {} items.",
                            Self::batch_limit(), data.len()
                        ),
                    ),
                );
            }
            let txn = db.begin().await?;
            let result: Vec<Self> = if db.support_returning() {
                let active_models: Vec<Self::ActiveModelType> = data
                    .into_iter()
                    .map(Into::into)
                    .collect();
                Self::EntityType::insert_many(active_models)
                    .exec_with_returning(&txn)
                    .await?
                    .into_iter()
                    .map(Self::from)
                    .collect()
            } else {
                let mut result = Vec::with_capacity(data.len());
                for create_model in data {
                    let active_model: Self::ActiveModelType = create_model.into();
                    let model = active_model.insert(&txn).await?;
                    result.push(Self::from(model));
                }
                result
            };
            txn.commit().await?;
            Ok(result)
        }
        async fn update(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
            data: Self::UpdateModel,
        ) -> Result<Self, crudcrate::ApiError> {
            {
                use crudcrate::validation::__auto::ValidatableFallback as _;
                crudcrate::validation::__auto::Probe(&data)
                    .crudcrate_auto_validate()
                    .map_err(crudcrate::ApiError::from)?;
            }
            use sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
            use crudcrate::traits::MergeIntoActiveModel;
            let model = Self::EntityType::find_by_id(id.clone())
                .one(db)
                .await?
                .ok_or_else(|| crudcrate::ApiError::not_found(
                    Self::RESOURCE_NAME_SINGULAR,
                    Some(id.to_string()),
                ))?;
            let existing: Self::ActiveModelType = model.into_active_model();
            let updated_model = data.merge_into_activemodel(existing)?;
            let updated = updated_model.update(db).await?;
            let result = Self::from(updated);
            Ok(result)
        }
        async fn update_many(
            db: &sea_orm::DatabaseConnection,
            updates: Vec<(crudcrate::PrimaryKeyType<Self>, Self::UpdateModel)>,
        ) -> Result<Vec<Self>, crudcrate::ApiError> {
            {
                use crudcrate::validation::__auto::ValidatableFallback as _;
                for (_, __cc_item) in &updates {
                    crudcrate::validation::__auto::Probe(__cc_item)
                        .crudcrate_auto_validate()
                        .map_err(crudcrate::ApiError::from)?;
                }
            }
            use sea_orm::{
                EntityTrait, IntoActiveModel, ActiveModelTrait, TransactionTrait,
            };
            use crudcrate::traits::MergeIntoActiveModel;
            if updates.len() > Self::batch_limit() {
                return Err(
                    crudcrate::ApiError::bad_request(
                        format!(
                            "Batch update limited to {} items. Received {} items.",
                            Self::batch_limit(), updates.len()
                        ),
                    ),
                );
            }
            let txn = db.begin().await?;
            let mut result = Vec::with_capacity(updates.len());
            for (id, update_model) in updates {
                let model = Self::EntityType::find_by_id(id.clone())
                    .one(&txn)
                    .await?
                    .ok_or_else(|| crudcrate::ApiError::not_found(
                        Self::RESOURCE_NAME_SINGULAR,
                        Some(id.to_string()),
                    ))?;
                let existing: Self::ActiveModelType = model.into_active_model();
                let updated_model = update_model.merge_into_activemodel(existing)?;
                let updated = updated_model.update(&txn).await?;
                result.push(Self::from(updated));
            }
            txn.commit().await?;
            Ok(result)
        }
        async fn delete(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
        ) -> Result<crudcrate::PrimaryKeyType<Self>, crudcrate::ApiError> {
            use sea_orm::EntityTrait;
            let res = Self::EntityType::delete_by_id(id.clone()).exec(db).await?;
            let result = match res.rows_affected {
                0 => {
                    return Err(
                        crudcrate::ApiError::not_found(
                            Self::RESOURCE_NAME_SINGULAR,
                            Some(id.to_string()),
                        ),
                    );
                }
                _ => id,
            };
            Ok(result)
        }
        async fn delete_many(
            db: &sea_orm::DatabaseConnection,
            ids: Vec<crudcrate::PrimaryKeyType<Self>>,
        ) -> Result<Vec<crudcrate::PrimaryKeyType<Self>>, crudcrate::ApiError> {
            use sea_orm::{EntityTrait, QueryFilter, QuerySelect, ColumnTrait};
            if ids.len() > Self::batch_limit() {
                return Err(
                    crudcrate::ApiError::bad_request(
                        format!(
                            "Batch delete limited to {} items. Received {} items.",
                            Self::batch_limit(), ids.len()
                        ),
                    ),
                );
            }
            let result = if ids.is_empty() {
                vec![]
            } else {
                let existing: Vec<crudcrate::PrimaryKeyType<Self>> = Self::EntityType::find()
                    .select_only()
                    .column(Self::ID_COLUMN)
                    .filter(Self::ID_COLUMN.is_in(ids.clone()))
                    .into_tuple::<crudcrate::PrimaryKeyType<Self>>()
                    .all(db)
                    .await?;
                let existing_set: std::collections::HashSet<
                    crudcrate::PrimaryKeyType<Self>,
                > = existing.into_iter().collect();
                if !existing_set.is_empty() {
                    Self::EntityType::delete_many()
                        .filter(
                            Self::ID_COLUMN
                                .is_in(existing_set.iter().cloned().collect::<Vec<_>>()),
                        )
                        .exec(db)
                        .await?;
                }
                let mut seen = std::collections::HashSet::new();
                ids.into_iter()
                    .filter(|id| existing_set.contains(id) && seen.insert(id.clone()))
                    .collect()
            };
            Ok(result)
        }
    }
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct CategoryList {
        pub id: Uuid,
        pub name: String,
        pub parent_id: Option<Uuid>,
        pub children: Vec<Category>,
    }
    impl From<Category> for CategoryList {
        fn from(model: Category) -> Self {
            Self {
                id: model.id,
                name: model.name,
                parent_id: model.parent_id,
                children: model.children.into_iter().map(Into::into).collect(),
            }
        }
    }
    impl From<Model> for CategoryList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                parent_id: model.parent_id,
                children: vec![],
            }
        }
    }
    pub type CategoryScopedList = CategoryList;
    pub type CategoryScopedResponse = CategoryResponse;
    impl crudcrate::ScopeFilterable for CategoryList {}
    impl crudcrate::ScopeFilterable for Category {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct CategoryResponse {
        pub id: Uuid,
        pub name: String,
        pub parent_id: Option<Uuid>,
        #[schema(no_recursion)]
        pub children: Vec<Category>,
    }
    impl From<Category> for CategoryResponse {
        fn from(model: Category) -> Self {
            Self {
                id: model.id,
                name: model.name,
                parent_id: model.parent_id,
                children: model.children,
            }
        }
    }
}
