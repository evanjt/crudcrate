mod vehicle__Model {
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
    pub struct Vehicle {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub customer_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub make: String,
    }
    impl From<Model> for Vehicle {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                customer_id: model.customer_id,
                make: model.make,
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Vehicle {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = VehicleCreate;
        type UpdateModel = VehicleUpdate;
        type ListModel = VehicleList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "vehicles";
        const RESOURCE_NAME_PLURAL: &'static str = "vehicles";
        const TABLE_NAME: &'static str = "vehicles";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages vehicles items";
        const FULLTEXT_LANGUAGE: &'static str = "english";
        fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("make", Self::ColumnType::Make)]
        }
        fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
            vec![("make", Self::ColumnType::Make)]
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
                "customer_id" => {
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
                "make" => {
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
            vec!["make"]
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
    pub struct VehicleList {
        pub id: Uuid,
        pub customer_id: Uuid,
        pub make: String,
    }
    impl From<Vehicle> for VehicleList {
        fn from(model: Vehicle) -> Self {
            Self {
                id: model.id,
                customer_id: model.customer_id,
                make: model.make,
            }
        }
    }
    impl From<Model> for VehicleList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                customer_id: model.customer_id,
                make: model.make,
            }
        }
    }
    pub type VehicleScopedList = VehicleList;
    pub type VehicleScopedResponse = VehicleResponse;
    impl crudcrate::ScopeFilterable for VehicleList {}
    impl crudcrate::ScopeFilterable for Vehicle {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct VehicleResponse {
        pub id: Uuid,
        pub customer_id: Uuid,
        pub make: String,
    }
    impl From<Vehicle> for VehicleResponse {
        fn from(model: Vehicle) -> Self {
            Self {
                id: model.id,
                customer_id: model.customer_id,
                make: model.make,
            }
        }
    }
}
mod customer__Model {
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
    pub struct Customer {
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        #[schema(no_recursion)]
        #[crudcrate(
            non_db_attr,
            exclude(create, update),
            join(one, all, depth = 1, filterable("make"), sortable("make"))
        )]
        pub vehicles: Vec<super::vehicle::Vehicle>,
    }
    impl From<Model> for Customer {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                vehicles: vec![],
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Customer {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = CustomerCreate;
        type UpdateModel = CustomerUpdate;
        type ListModel = CustomerList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        fn pk_value(
            model: &<Self::EntityType as sea_orm::EntityTrait>::Model,
        ) -> crudcrate::PrimaryKeyType<Self> {
            model.id.clone()
        }
        const RESOURCE_NAME_SINGULAR: &'static str = "customers";
        const RESOURCE_NAME_PLURAL: &'static str = "customers";
        const TABLE_NAME: &'static str = "customers";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages customers items";
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
            vec![
                crudcrate::JoinedColumnDef { join_field : "vehicles", column_name :
                "make", full_path : "vehicles.make", }
            ]
        }
        fn joined_sortable_columns() -> Vec<crudcrate::JoinedColumnDef> {
            vec![
                crudcrate::JoinedColumnDef { join_field : "vehicles", column_name :
                "make", full_path : "vehicles.make", }
            ]
        }
        async fn resolve_joined_filters(
            _db: &sea_orm::DatabaseConnection,
            condition: sea_orm::Condition,
            joined_filters: &[crudcrate::JoinedFilter],
        ) -> Result<sea_orm::Condition, crudcrate::ApiError> {
            if joined_filters.is_empty() {
                return Ok(condition);
            }
            let mut __augmented = condition;
            for __jf in joined_filters {
                match __jf.join_field.as_str() {
                    "vehicles" => {
                        let __sub_expr: Option<sea_orm::sea_query::Expr> = match __jf
                            .column
                            .as_str()
                        {
                            "make" => {
                                crudcrate::build_comparison_expr(
                                    super::vehicle::Column::Make,
                                    __jf.operator,
                                    &__jf.value,
                                )
                            }
                            _ => None,
                        };
                        if let Some(__sub_expr) = __sub_expr {
                            use sea_orm::ExprTrait;
                            use sea_orm::sea_query::{Expr, Query, QueryStatementBuilder};
                            let __child_scope: Option<sea_orm::Condition> = <super::vehicle::VehicleList as crudcrate::ScopeFilterable>::scope_condition();
                            let mut __where = sea_orm::Condition::all().add(__sub_expr);
                            if let Some(__cs) = __child_scope {
                                __where = __where.add(__cs);
                            }
                            let __fk_ref: sea_orm::sea_query::ColumnRef = {
                                let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                                    <Self as crudcrate::traits::CRUDResource>::EntityType,
                                >>::to();
                                let __fk_col_name = sea_orm::Iden::to_string(
                                    &__rel_def.from_col,
                                );
                                let __child_tbl = sea_orm::EntityName::table_name(
                                        &super::vehicle::Entity,
                                    )
                                    .to_string();
                                crudcrate::table_column_ref(__child_tbl, __fk_col_name)
                            };
                            let __subquery = Query::select()
                                .expr(Expr::col(__fk_ref))
                                .from(super::vehicle::Entity)
                                .cond_where(__where)
                                .to_owned();
                            let (__pt, __pc) = sea_orm::ColumnTrait::as_column_ref(
                                &Self::ID_COLUMN,
                            );
                            let __parent_ref = crudcrate::table_column_ref(__pt, __pc);
                            __augmented = __augmented
                                .add(Expr::col(__parent_ref).in_subquery(__subquery));
                        }
                    }
                    _ => {}
                }
            }
            Ok(__augmented)
        }
        fn joined_field_has_scope(field: &str) -> bool {
            match field {
                "vehicles" => {
                    <super::vehicle::VehicleList as crudcrate::ScopeFilterable>::scope_condition()
                        .is_some()
                }
                _ => false,
            }
        }
        async fn get_all_joined_sorted(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            join_field: &str,
            column: &str,
            direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            use sea_orm::{
                EntityTrait, ColumnTrait, QueryFilter, QueryOrder, QuerySelect,
            };
            match join_field {
                "vehicles" => {
                    let __child_col_ref: Option<sea_orm::sea_query::ColumnRef> = match column {
                        "make" => {
                            let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                                &super::vehicle::Column::Make,
                            );
                            Some(crudcrate::table_column_ref(__t, __c))
                        }
                        _ => None,
                    };
                    if let Some(__child_col_ref) = __child_col_ref {
                        use sea_orm::ExprTrait;
                        use sea_orm::sea_query::{Expr, Query, QueryStatementBuilder};
                        let __parent_pk_ref = <Self::ColumnType as sea_orm::ColumnTrait>::as_column_ref(
                            &Self::ID_COLUMN,
                        );
                        let __fk_ref: sea_orm::sea_query::ColumnRef = {
                            let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                                <Self as crudcrate::traits::CRUDResource>::EntityType,
                            >>::to();
                            let __fk_col_name = sea_orm::Iden::to_string(
                                &__rel_def.from_col,
                            );
                            let __child_tbl = sea_orm::EntityName::table_name(
                                    &super::vehicle::Entity,
                                )
                                .to_string();
                            crudcrate::table_column_ref(__child_tbl, __fk_col_name)
                        };
                        let __subquery = Query::select()
                            .expr(Expr::col(__child_col_ref).min())
                            .from(super::vehicle::Entity)
                            .and_where(Expr::col(__fk_ref).equals(__parent_pk_ref))
                            .to_owned();
                        let __order_expr = sea_orm::sea_query::Expr::SubQuery(
                            None,
                            Box::new(__subquery.into_sub_query_statement()),
                        );
                        let __models = Self::EntityType::find()
                            .filter(condition.clone())
                            .order_by(__order_expr, direction)
                            .order_by(Self::ID_COLUMN, sea_orm::Order::Asc)
                            .offset(offset)
                            .limit(limit)
                            .all(db)
                            .await
                            .map_err(crudcrate::ApiError::database)?;
                        return Ok(
                            __models
                                .into_iter()
                                .map(|__m| <Self::ListModel as From<
                                    Self,
                                >>::from(Self::from(__m)))
                                .collect(),
                        );
                    }
                }
                _ => {}
            }
            Self::get_all(
                    db,
                    condition,
                    Self::default_index_column(),
                    direction,
                    offset,
                    limit,
                )
                .await
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
                    let loaded_vehicles: Vec<super::vehicle::Vehicle> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::vehicle::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let related_models = Box::pin(query.all(db)).await?;
                        related_models
                            .into_iter()
                            .map(|m: super::vehicle::Model| super::vehicle::Vehicle::from(
                                m,
                            ))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.vehicles = loaded_vehicles;
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
                    let loaded_vehicles: Vec<super::vehicle::Vehicle> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType,
                        >>::to();
                        let __fk_col_name = sea_orm::Iden::to_string(
                            &__rel_def.from_col,
                        );
                        let query = super::vehicle::Entity::find()
                            .filter(
                                sea_orm::sea_query::Expr::col(
                                        sea_orm::sea_query::Alias::new(&__fk_col_name),
                                    )
                                    .eq(
                                        <Self as crudcrate::traits::CRUDResource>::pk_value(&model),
                                    ),
                            );
                        let query = if let Some(child_scope) = <super::vehicle::VehicleList as crudcrate::ScopeFilterable>::scope_condition() {
                            query.filter(child_scope)
                        } else {
                            query
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        related_models
                            .into_iter()
                            .map(|m: super::vehicle::Model| super::vehicle::Vehicle::from(
                                m,
                            ))
                            .collect::<Vec<_>>()
                    };
                    let mut result: Self = model.into();
                    result.vehicles = loaded_vehicles;
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
            let mut vehicles_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<super::vehicle::Vehicle>,
            > = Box::pin(async {
                    use sea_orm::{
                        EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait,
                    };
                    use std::str::FromStr;
                    let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType,
                    >>::to();
                    let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                    let query = super::vehicle::Entity::find()
                        .filter(
                            sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name),
                                )
                                .is_in(parent_ids.clone()),
                        );
                    let all_related = query.all(db).await?;
                    let __fk_col = match <<super::vehicle::Entity as sea_orm::EntityTrait>::Column as FromStr>::from_str(
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
                        Vec<super::vehicle::Vehicle>,
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
                            .push(super::vehicle::Vehicle::from(related_model));
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.vehicles = vehicles_by_parent
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
            let mut vehicles_by_parent: std::collections::HashMap<
                crudcrate::PrimaryKeyType<Self>,
                Vec<super::vehicle::Vehicle>,
            > = Box::pin(async {
                    use sea_orm::{
                        EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait,
                    };
                    use std::str::FromStr;
                    let __rel_def = <super::vehicle::Entity as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType,
                    >>::to();
                    let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                    let query = super::vehicle::Entity::find()
                        .filter(
                            sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name),
                                )
                                .is_in(parent_ids.clone()),
                        );
                    let __child_scope: Option<sea_orm::Condition> = <super::vehicle::VehicleList as crudcrate::ScopeFilterable>::scope_condition();
                    let query = if let Some(ref cs) = __child_scope {
                        query.filter(cs.clone())
                    } else {
                        query
                    };
                    let all_related = query.all(db).await?;
                    let __fk_col = match <<super::vehicle::Entity as sea_orm::EntityTrait>::Column as FromStr>::from_str(
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
                        Vec<super::vehicle::Vehicle>,
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
                            .push(super::vehicle::Vehicle::from(related_model));
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                })
                .await?;
            let mut result = Vec::new();
            for model in models {
                let item = {
                    let parent_id = model.id.clone();
                    let mut item = Self::from(model);
                    item.vehicles = vehicles_by_parent
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
    #[cfg(test)]
    #[allow(non_snake_case)]
    mod _crudcrate_fk_validation_customer {
        use super::*;
        #[test]
        fn _crudcrate_validate_fk_customer_vehicles() {
            let def = <super::super::vehicle::Entity as sea_orm::Related<
                super::Entity,
            >>::to();
            let from_col_name = sea_orm::Iden::to_string(&def.from_col);
            if from_col_name != "customer_id" {
                eprintln!(
                    "crudcrate: FK for 'Customer.vehicles': convention='customer_id', actual='{}' (resolved from SeaORM RelationDef at runtime)",
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
    pub struct CustomerList {
        pub id: Uuid,
        pub name: String,
        pub vehicles: Vec<super::vehicle::VehicleList>,
    }
    impl From<Customer> for CustomerList {
        fn from(model: Customer) -> Self {
            Self {
                id: model.id,
                name: model.name,
                vehicles: model.vehicles.into_iter().map(Into::into).collect(),
            }
        }
    }
    impl From<Model> for CustomerList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                vehicles: vec![],
            }
        }
    }
    pub type CustomerScopedList = CustomerList;
    pub type CustomerScopedResponse = CustomerResponse;
    impl crudcrate::ScopeFilterable for CustomerList {}
    impl crudcrate::ScopeFilterable for Customer {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct CustomerResponse {
        pub id: Uuid,
        pub name: String,
        #[schema(no_recursion)]
        pub vehicles: Vec<super::vehicle::Vehicle>,
    }
    impl From<Customer> for CustomerResponse {
        fn from(model: Customer) -> Self {
            Self {
                id: model.id,
                name: model.name,
                vehicles: model.vehicles,
            }
        }
    }
    #[doc(hidden)]
    pub const _BIDIRECTIONAL_RELATION_CUSTOMER_VEHICLES: bool = crudcrate::impls!(
        super::vehicle::Entity : sea_orm::Related < Entity >
    );
}
