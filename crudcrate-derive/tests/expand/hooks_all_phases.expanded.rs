mod Model {
    use sea_orm::ActiveValue;
    #[derive(
        Clone,
        Debug,
        serde::Serialize,
        serde::Deserialize,
        crudcrate::ToCreateModel,
        crudcrate::ToUpdateModel,
        utoipa::ToSchema,
        Default
    )]
    #[active_model = "ActiveModel"]
    pub struct Item {
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        #[crudcrate(exclude(create, update), on_create = Utc::now())]
        pub created_at: DateTime<Utc>,
    }
    impl From<Model> for Item {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                created_at: model.created_at,
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Item {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = ItemCreate;
        type UpdateModel = ItemUpdate;
        type ListModel = ItemList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "item";
        const RESOURCE_NAME_PLURAL: &'static str = "items";
        const TABLE_NAME: &'static str = "items";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages item items";
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
                "created_at" => {
                    trait __Fallback {
                        const V: bool = false;
                    }
                    impl<T> __Fallback for __Probe<T> {}
                    struct __Probe<T>(::core::marker::PhantomData<T>);
                    #[allow(dead_code)]
                    impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                        const V: bool = true;
                    }
                    <__Probe<DateTime<Utc>>>::V
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
            pre_read(db, id).await?;
            let model = Self::EntityType::find_by_id(id.clone()).one(db).await?;
            let mut result = match model {
                Some(model) => Self::from(model),
                None => {
                    return Err(
                        crudcrate::ApiError::not_found(
                            Self::RESOURCE_NAME_SINGULAR,
                            Some(id.to_string()),
                        ),
                    );
                }
            };
            let result = transform_read(db, result).await?;
            post_read(db, &result).await?;
            Ok(result)
        }
        async fn get_one_scoped(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
            scope: &sea_orm::Condition,
        ) -> Result<Self, crudcrate::ApiError> {
            pre_read(db, id).await?;
            use sea_orm::QueryFilter;
            let scoped_condition = sea_orm::Condition::all()
                .add(Self::ID_COLUMN.eq(id.clone()))
                .add(scope.clone());
            let model = Self::EntityType::find().filter(scoped_condition).one(db).await?;
            let mut result = match model {
                Some(model) => Self::from(model),
                None => {
                    return Err(
                        crudcrate::ApiError::not_found(
                            Self::RESOURCE_NAME_SINGULAR,
                            Some(id.to_string()),
                        ),
                    );
                }
            };
            let result = transform_read(db, result).await?;
            post_read(db, &result).await?;
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
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait, IdenStatic};
            let mut __query = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction);
            if order_column.as_str() != Self::ID_COLUMN.as_str() {
                __query = __query.order_by(Self::ID_COLUMN, sea_orm::Order::Asc);
            }
            let models = __query.offset(offset).limit(limit).all(db).await?;
            let result: Vec<Self::ListModel> = models
                .into_iter()
                .map(|model| Self::ListModel::from(Self::from(model)))
                .collect();
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
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait, IdenStatic};
            let mut __query = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction);
            if order_column.as_str() != Self::ID_COLUMN.as_str() {
                __query = __query.order_by(Self::ID_COLUMN, sea_orm::Order::Asc);
            }
            let models = __query.offset(offset).limit(limit).all(db).await?;
            let result: Vec<Self::ListModel> = models
                .into_iter()
                .map(|model| Self::ListModel::from(Self::from(model)))
                .collect();
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
            pre_create(db, &data).await?;
            let active_model: Self::ActiveModelType = data.into();
            let insert_result = Self::EntityType::insert(active_model).exec(db).await?;
            let result = Self::get_one(db, insert_result.last_insert_id.into()).await?;
            let result = transform_create(db, result).await?;
            post_create(db, &result).await?;
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
            pre_update(db, id.clone(), &data).await?;
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
            let result = transform_update(db, result).await?;
            post_update(db, &result).await?;
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
            pre_delete(db, id.clone()).await?;
            let result = body_delete(db, id).await?;
            let result = transform_delete(db, result).await?;
            post_delete(db, result.clone()).await?;
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
    crudcrate::crud_handlers!(
        Item, ItemUpdate, ItemCreate, ItemList, ItemResponse, ItemList, ItemResponse
    );
    impl Item {
        /// Generate router with all CRUD endpoints
        pub fn router(
            db: &sea_orm::DatabaseConnection,
        ) -> utoipa_axum::router::OpenApiRouter
        where
            Self: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};
            crudcrate::tracing::info!(
                resource = Self::RESOURCE_NAME_PLURAL, table = Self::TABLE_NAME,
                batch_limit = Self::batch_limit(), max_page_size = Self::max_page_size(),
                "Mounting CRUD routes with security defaults: input_sanitization=enabled, sql_parameterization=enabled. See https://crudcrate.evanjt.com/latest/advanced/security.html"
            );
            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(create_many_handler))
                .routes(routes!(update_one_handler))
                .routes(routes!(update_many_handler))
                .routes(routes!(delete_one_handler))
                .routes(routes!(delete_many_handler))
                .layer(
                    axum::extract::DefaultBodyLimit::max(
                        Self::security_profile().max_request_body_bytes,
                    ),
                )
                .with_state(db.clone())
        }
        /// Generate read-only router with only GET endpoints.
        ///
        /// Use with [`ScopeCondition`](crudcrate::ScopeCondition) to create
        /// public/filtered API endpoints:
        ///
        /// ```rust,ignore
        /// use crudcrate::ScopeCondition;
        ///
        /// let public = Article::read_only_router(&db)
        ///     .layer(Extension(ScopeCondition(
        ///         Condition::all().add(article::Column::IsPrivate.eq(false))
        ///     )));
        /// ```
        pub fn read_only_router(
            db: &sea_orm::DatabaseConnection,
        ) -> utoipa_axum::router::OpenApiRouter
        where
            Self: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};
            crudcrate::tracing::info!(
                resource = Self::RESOURCE_NAME_PLURAL, table = Self::TABLE_NAME,
                max_page_size = Self::max_page_size(), "Mounting read-only routes"
            );
            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .layer(
                    axum::extract::DefaultBodyLimit::max(
                        Self::security_profile().max_request_body_bytes,
                    ),
                )
                .with_state(db.clone())
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
    pub struct ItemList {
        pub id: Uuid,
        pub name: String,
        pub created_at: DateTime<Utc>,
    }
    impl From<Item> for ItemList {
        fn from(model: Item) -> Self {
            Self {
                id: model.id,
                name: model.name,
                created_at: model.created_at,
            }
        }
    }
    impl From<Model> for ItemList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                created_at: model.created_at,
            }
        }
    }
    pub type ItemScopedList = ItemList;
    pub type ItemScopedResponse = ItemResponse;
    impl crudcrate::ScopeFilterable for ItemList {}
    impl crudcrate::ScopeFilterable for Item {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct ItemResponse {
        pub id: Uuid,
        pub name: String,
        pub created_at: DateTime<Utc>,
    }
    impl From<Item> for ItemResponse {
        fn from(model: Item) -> Self {
            Self {
                id: model.id,
                name: model.name,
                created_at: model.created_at,
            }
        }
    }
}
