mod part__Model {
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
    pub struct Part {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub machine_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
    }
    impl From<Model> for Part {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                name: model.name,
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Part {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = PartCreate;
        type UpdateModel = PartUpdate;
        type ListModel = PartList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "parts";
        const RESOURCE_NAME_PLURAL: &'static str = "parts";
        const TABLE_NAME: &'static str = "parts";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages parts items";
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
                "machine_id" => {
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
            Ok(result)
        }
        async fn get_one_scoped(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
            scope: &sea_orm::Condition,
        ) -> Result<Self, crudcrate::ApiError> {
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
    pub struct PartList {
        pub id: Uuid,
        pub machine_id: Uuid,
        pub name: String,
    }
    impl From<Part> for PartList {
        fn from(model: Part) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                name: model.name,
            }
        }
    }
    impl From<Model> for PartList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                name: model.name,
            }
        }
    }
    pub type PartScopedList = PartList;
    pub type PartScopedResponse = PartResponse;
    impl crudcrate::ScopeFilterable for PartList {}
    impl crudcrate::ScopeFilterable for Part {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct PartResponse {
        pub id: Uuid,
        pub machine_id: Uuid,
        pub name: String,
    }
    impl From<Part> for PartResponse {
        fn from(model: Part) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                name: model.name,
            }
        }
    }
}
mod log__Model {
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
    pub struct Log {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub machine_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub message: String,
    }
    impl From<Model> for Log {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                message: model.message,
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Log {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = LogCreate;
        type UpdateModel = LogUpdate;
        type ListModel = LogList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "logs";
        const RESOURCE_NAME_PLURAL: &'static str = "logs";
        const TABLE_NAME: &'static str = "logs";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages logs items";
        const FULLTEXT_LANGUAGE: &'static str = "english";
        fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("message", Self::ColumnType::Message)]
        }
        fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("message", Self::ColumnType::Message)]
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
                "machine_id" => {
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
                "message" => {
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
                _ => false,
            }
        }
        fn like_filterable_columns() -> Vec<&'static str> {
            vec!["message"]
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
            Ok(result)
        }
        async fn get_one_scoped(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
            scope: &sea_orm::Condition,
        ) -> Result<Self, crudcrate::ApiError> {
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
    pub struct LogList {
        pub id: Uuid,
        pub machine_id: Uuid,
        pub message: String,
    }
    impl From<Log> for LogList {
        fn from(model: Log) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                message: model.message,
            }
        }
    }
    impl From<Model> for LogList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                message: model.message,
            }
        }
    }
    pub type LogScopedList = LogList;
    pub type LogScopedResponse = LogResponse;
    impl crudcrate::ScopeFilterable for LogList {}
    impl crudcrate::ScopeFilterable for Log {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct LogResponse {
        pub id: Uuid,
        pub machine_id: Uuid,
        pub message: String,
    }
    impl From<Log> for LogResponse {
        fn from(model: Log) -> Self {
            Self {
                id: model.id,
                machine_id: model.machine_id,
                message: model.message,
            }
        }
    }
}
mod machine__Model {
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
    pub struct Machine {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub label: String,
        #[schema(no_recursion)]
        #[crudcrate(non_db_attr, join(one, depth = 1))]
        pub parts: Vec<super::part::Part>,
        #[schema(no_recursion)]
        #[crudcrate(non_db_attr, join(all, depth = 1))]
        pub logs: Vec<super::log::Log>,
    }
    impl From<Model> for Machine {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                label: model.label,
                parts: vec![],
                logs: vec![],
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Machine {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = MachineCreate;
        type UpdateModel = MachineUpdate;
        type ListModel = MachineList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "machines";
        const RESOURCE_NAME_PLURAL: &'static str = "machines";
        const TABLE_NAME: &'static str = "machines";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages machines items";
        const FULLTEXT_LANGUAGE: &'static str = "english";
        fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("label", Self::ColumnType::Label)]
        }
        fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("label", Self::ColumnType::Label)]
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
                "label" => {
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
                _ => false,
            }
        }
        fn like_filterable_columns() -> Vec<&'static str> {
            vec!["label"]
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
                    let loaded_parts: Vec<super::part::Part> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::part::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::part::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile
                            .check_child_rows(
                                related_models.len(),
                                <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                                "parts",
                            )?;
                        related_models
                            .into_iter()
                            .map(|m: super::part::Model| super::part::Part::from(m))
                            .collect::<Vec<_>>()
                    };
                    let loaded_logs: Vec<super::log::Log> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::log::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::log::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile
                            .check_child_rows(
                                related_models.len(),
                                <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                                "logs",
                            )?;
                        related_models
                            .into_iter()
                            .map(|m: super::log::Model| super::log::Log::from(m))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.parts = loaded_parts;
                    result.logs = loaded_logs;
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
                    let loaded_parts: Vec<super::part::Part> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::part::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::part::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let query = if let Some(child_scope) = <super::part::PartList as crudcrate::ScopeFilterable>::scope_condition() {
                            query.filter(child_scope)
                        } else {
                            query
                        };
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile
                            .check_child_rows(
                                related_models.len(),
                                <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                                "parts",
                            )?;
                        related_models
                            .into_iter()
                            .map(|m: super::part::Model| super::part::Part::from(m))
                            .collect::<Vec<_>>()
                    };
                    let loaded_logs: Vec<super::log::Log> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::log::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::log::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let query = if let Some(child_scope) = <super::log::LogList as crudcrate::ScopeFilterable>::scope_condition() {
                            query.filter(child_scope)
                        } else {
                            query
                        };
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile
                            .check_child_rows(
                                related_models.len(),
                                <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                                "logs",
                            )?;
                        related_models
                            .into_iter()
                            .map(|m: super::log::Model| super::log::Log::from(m))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.parts = loaded_parts;
                    result.logs = loaded_logs;
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
            let mut logs_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<super::log::Log>,
            > = Box::pin(async {
                    use sea_orm::{
                        EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait,
                    };
                    use std::str::FromStr;
                    let __rel_def = <super::log::Entity as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType,
                    >>::to();
                    let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                    let query = super::log::Entity::find()
                        .filter(
                            sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name),
                                )
                                .is_in(parent_ids.clone()),
                        );
                    let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                    let query = match __profile.child_row_limit() {
                        Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                        None => query,
                    };
                    let all_related = query.all(db).await?;
                    __profile
                        .check_child_rows(
                            all_related.len(),
                            <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                            "logs",
                        )?;
                    let __fk_col = match <<super::log::Entity as sea_orm::EntityTrait>::Column as FromStr>::from_str(
                        &__fk_col_name,
                    ) {
                        Ok(__c) => __c,
                        Err(_) => {
                            return Err(
                                crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                ),
                            );
                        }
                    };
                    let mut map: std::collections::HashMap<
                        crudcrate::PrimaryKeyType<Self>,
                        Vec<super::log::Log>,
                    > = std::collections::HashMap::new();
                    for related_model in all_related {
                        let fk_value: crudcrate::PrimaryKeyType<Self> = match <crudcrate::PrimaryKeyType<
                            Self,
                        > as sea_orm::sea_query::ValueType>::try_from(
                            ModelTrait::get(&related_model, __fk_col.clone()),
                        ) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        map.entry(fk_value)
                            .or_insert_with(Vec::new)
                            .push(super::log::Log::from(related_model));
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.logs = logs_by_parent.remove(&parent_id).unwrap_or_default();
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
            let mut logs_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<super::log::Log>,
            > = Box::pin(async {
                    use sea_orm::{
                        EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait,
                    };
                    use std::str::FromStr;
                    let __rel_def = <super::log::Entity as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType,
                    >>::to();
                    let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                    let query = super::log::Entity::find()
                        .filter(
                            sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name),
                                )
                                .is_in(parent_ids.clone()),
                        );
                    let __child_scope: Option<sea_orm::Condition> = <super::log::LogList as crudcrate::ScopeFilterable>::scope_condition();
                    let query = if let Some(ref cs) = __child_scope {
                        query.filter(cs.clone())
                    } else {
                        query
                    };
                    let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                    let query = match __profile.child_row_limit() {
                        Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                        None => query,
                    };
                    let all_related = query.all(db).await?;
                    __profile
                        .check_child_rows(
                            all_related.len(),
                            <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                            "logs",
                        )?;
                    let __fk_col = match <<super::log::Entity as sea_orm::EntityTrait>::Column as FromStr>::from_str(
                        &__fk_col_name,
                    ) {
                        Ok(__c) => __c,
                        Err(_) => {
                            return Err(
                                crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                ),
                            );
                        }
                    };
                    let mut map: std::collections::HashMap<
                        crudcrate::PrimaryKeyType<Self>,
                        Vec<super::log::Log>,
                    > = std::collections::HashMap::new();
                    for related_model in all_related {
                        let fk_value: crudcrate::PrimaryKeyType<Self> = match <crudcrate::PrimaryKeyType<
                            Self,
                        > as sea_orm::sea_query::ValueType>::try_from(
                            ModelTrait::get(&related_model, __fk_col.clone()),
                        ) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        map.entry(fk_value)
                            .or_insert_with(Vec::new)
                            .push(super::log::Log::from(related_model));
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.logs = logs_by_parent.remove(&parent_id).unwrap_or_default();
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
    #[cfg(test)]
    #[allow(non_snake_case)]
    mod _crudcrate_fk_validation_machine {
        use super::*;
        #[test]
        fn _crudcrate_validate_fk_machine_parts() {
            let def = <super::super::part::Entity as sea_orm::Related<
                super::Entity,
            >>::to();
            let from_col_name = sea_orm::Iden::to_string(&def.from_col);
            if from_col_name != "machine_id" {
                eprintln!(
                    "crudcrate: FK for 'Machine.parts': convention='machine_id', actual='{}' (resolved from SeaORM RelationDef at runtime)",
                    from_col_name
                );
            }
        }
        #[test]
        fn _crudcrate_validate_fk_machine_logs() {
            let def = <super::super::log::Entity as sea_orm::Related<
                super::Entity,
            >>::to();
            let from_col_name = sea_orm::Iden::to_string(&def.from_col);
            if from_col_name != "machine_id" {
                eprintln!(
                    "crudcrate: FK for 'Machine.logs': convention='machine_id', actual='{}' (resolved from SeaORM RelationDef at runtime)",
                    from_col_name
                );
            }
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
    pub struct MachineList {
        pub id: Uuid,
        pub label: String,
        pub logs: Vec<super::log::LogList>,
    }
    impl From<Machine> for MachineList {
        fn from(model: Machine) -> Self {
            Self {
                id: model.id,
                label: model.label,
                logs: model.logs.into_iter().map(Into::into).collect(),
            }
        }
    }
    impl From<Model> for MachineList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                label: model.label,
                logs: vec![],
            }
        }
    }
    pub type MachineScopedList = MachineList;
    pub type MachineScopedResponse = MachineResponse;
    impl crudcrate::ScopeFilterable for MachineList {}
    impl crudcrate::ScopeFilterable for Machine {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct MachineResponse {
        pub id: Uuid,
        pub label: String,
        #[schema(no_recursion)]
        pub parts: Vec<super::part::Part>,
        #[schema(no_recursion)]
        pub logs: Vec<super::log::Log>,
    }
    impl From<Machine> for MachineResponse {
        fn from(model: Machine) -> Self {
            Self {
                id: model.id,
                label: model.label,
                parts: model.parts,
                logs: model.logs,
            }
        }
    }
    #[doc(hidden)]
    pub const _BIDIRECTIONAL_RELATION_MACHINE_PARTS: bool = crudcrate::impls!(
        super::part::Entity : sea_orm::Related < Entity >
    );
    #[doc(hidden)]
    pub const _BIDIRECTIONAL_RELATION_MACHINE_LOGS: bool = crudcrate::impls!(
        super::log::Entity : sea_orm::Related < Entity >
    );
}
