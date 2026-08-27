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
    pub struct Product {
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        pub price: i32,
    }
    impl From<Model> for Product {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                price: model.price,
            }
        }
    }
    #[async_trait::async_trait]
    impl crudcrate::CRUDResource for Product {
        type EntityType = Entity;
        type ColumnType = Column;
        type ActiveModelType = ActiveModel;
        type CreateModel = ProductCreate;
        type UpdateModel = ProductUpdate;
        type ListModel = ProductList;
        const ID_COLUMN: Self::ColumnType = Self::ColumnType::Id;
        const RESOURCE_NAME_SINGULAR: &'static str = "products";
        const RESOURCE_NAME_PLURAL: &'static str = "products";
        const TABLE_NAME: &'static str = "products";
        const RESOURCE_DESCRIPTION: &'static str = "This resource manages products items";
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
                "price" => {
                    trait __Fallback {
                        const V: bool = false;
                    }
                    impl<T> __Fallback for __Probe<T> {}
                    struct __Probe<T>(::core::marker::PhantomData<T>);
                    #[allow(dead_code)]
                    impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                        const V: bool = true;
                    }
                    <__Probe<i32>>::V
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
            let ops = ProductOperations;
            crudcrate::CRUDOperations::get_one(&ops, db, id).await
        }
        async fn get_all(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            let ops = ProductOperations;
            crudcrate::CRUDOperations::get_all(
                    &ops,
                    db,
                    condition,
                    order_column,
                    order_direction,
                    offset,
                    limit,
                )
                .await
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
            let ops = ProductOperations;
            crudcrate::CRUDOperations::create(&ops, db, data).await
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
            let ops = ProductOperations;
            crudcrate::CRUDOperations::create_many(&ops, db, data).await
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
            let ops = ProductOperations;
            crudcrate::CRUDOperations::update(&ops, db, id, data).await
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
            let ops = ProductOperations;
            crudcrate::CRUDOperations::update_many(&ops, db, updates).await
        }
        async fn delete(
            db: &sea_orm::DatabaseConnection,
            id: crudcrate::PrimaryKeyType<Self>,
        ) -> Result<crudcrate::PrimaryKeyType<Self>, crudcrate::ApiError> {
            let ops = ProductOperations;
            crudcrate::CRUDOperations::delete(&ops, db, id).await
        }
        async fn delete_many(
            db: &sea_orm::DatabaseConnection,
            ids: Vec<crudcrate::PrimaryKeyType<Self>>,
        ) -> Result<Vec<crudcrate::PrimaryKeyType<Self>>, crudcrate::ApiError> {
            let ops = ProductOperations;
            crudcrate::CRUDOperations::delete_many(&ops, db, ids).await
        }
    }
    crudcrate::crud_handlers!(
        Product, ProductUpdate, ProductCreate, ProductList, ProductResponse, ProductList,
        ProductResponse
    );
    impl Product {
        /// Generate router with all CRUD endpoints
        pub fn router(
            db: &sea_orm::DatabaseConnection,
        ) -> utoipa_axum::router::OpenApiRouter
        where
            Self: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};
            tracing::info!(
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
            tracing::info!(
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
    pub struct ProductList {
        pub id: Uuid,
        pub name: String,
        pub price: i32,
    }
    impl From<Product> for ProductList {
        fn from(model: Product) -> Self {
            Self {
                id: model.id,
                name: model.name,
                price: model.price,
            }
        }
    }
    impl From<Model> for ProductList {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                name: model.name,
                price: model.price,
            }
        }
    }
    pub type ProductScopedList = ProductList;
    pub type ProductScopedResponse = ProductResponse;
    impl crudcrate::ScopeFilterable for ProductList {}
    impl crudcrate::ScopeFilterable for Product {}
    #[derive(
        Clone,
        Debug,
        PartialEq,
        serde::Serialize,
        serde::Deserialize,
        utoipa::ToSchema
    )]
    pub struct ProductResponse {
        pub id: Uuid,
        pub name: String,
        pub price: i32,
    }
    impl From<Product> for ProductResponse {
        fn from(model: Product) -> Self {
            Self {
                id: model.id,
                name: model.name,
                price: model.price,
            }
        }
    }
}
