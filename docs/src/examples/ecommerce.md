# E-commerce Orders Example

Order management with products, customers, and line items.

## Entities

### Product

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub name: String,

    #[crudcrate(fulltext)]
    pub description: Option<String>,

    #[crudcrate(filterable, sortable)]
    pub price: Decimal,

    #[crudcrate(filterable)]
    pub category: String,

    #[crudcrate(filterable, sortable)]
    pub stock_quantity: i32,

    #[crudcrate(filterable)]
    pub active: bool,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

### Customer

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "customers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub email: String,

    pub phone: Option<String>,

    pub shipping_address: Option<String>,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    // Customer's orders
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub orders: Vec<super::order::Order>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::order::Entity")]
    Orders,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef { Relation::Orders.def() }
}
```

### Order

```rust
#[derive(Clone, Debug, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum OrderStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "confirmed")]
    Confirmed,
    #[sea_orm(string_value = "shipped")]
    Shipped,
    #[sea_orm(string_value = "delivered")]
    Delivered,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router, operations = OrderOperations)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub customer_id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub status: OrderStatus,

    #[crudcrate(sortable)]
    pub total_amount: Decimal,

    pub shipping_address: String,

    pub notes: Option<String>,

    #[crudcrate(sortable, filterable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTimeUtc,

    // Customer
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub customer: Option<super::customer::Customer>,

    // Line items
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub items: Vec<super::order_item::OrderItem>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::customer::Entity",
        from = "Column::CustomerId",
        to = "super::customer::Column::Id"
    )]
    Customer,

    #[sea_orm(has_many = "super::order_item::Entity")]
    Items,
}

impl Related<super::customer::Entity> for Entity {
    fn to() -> RelationDef { Relation::Customer.def() }
}

impl Related<super::order_item::Entity> for Entity {
    fn to() -> RelationDef { Relation::Items.def() }
}
```

### Order Item

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "order_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub order_id: Uuid,

    #[crudcrate(filterable)]
    pub product_id: Uuid,

    pub quantity: i32,

    pub unit_price: Decimal,

    pub total_price: Decimal,

    // Product details at time of order
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub product: Option<super::product::Product>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id"
    )]
    Order,

    #[sea_orm(
        belongs_to = "super::product::Entity",
        from = "Column::ProductId",
        to = "super::product::Column::Id"
    )]
    Product,
}

impl Related<super::product::Entity> for Entity {
    fn to() -> RelationDef { Relation::Product.def() }
}
```

## Custom Operations

```rust
pub struct OrderOperations;

#[async_trait]
impl CRUDOperations for OrderOperations {
    type Resource = Order;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut OrderCreate,
    ) -> Result<(), ApiError> {
        // Calculate total from items
        // Validate stock availability
        Ok(())
    }

    async fn after_create(
        &self,
        db: &DatabaseConnection,
        created: &Order,
    ) -> Result<(), ApiError> {
        // Update stock quantities
        // Send confirmation email
        Ok(())
    }

    async fn before_update(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
        data: &mut OrderUpdate,
    ) -> Result<(), ApiError> {
        let order = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;

        // Prevent changing cancelled orders
        if order.status == OrderStatus::Cancelled {
            return Err(ApiError::BadRequest("Cannot modify cancelled order".into()));
        }

        // Validate status transitions
        if let Some(new_status) = &data.status {
            if !is_valid_transition(&order.status, new_status) {
                return Err(ApiError::BadRequest("Invalid status transition".into()));
            }
        }

        Ok(())
    }
}
```

## API Examples

### Create Order

```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "{customer-id}",
    "status": "pending",
    "total_amount": "99.99",
    "shipping_address": "123 Main St, City, Country"
  }'
```

### Get Order with Items

```bash
curl "http://localhost:3000/orders/{order-id}"

# Response
{
  "id": "...",
  "status": "confirmed",
  "customer": {"name": "John Doe", "email": "..."},
  "items": [
    {"product": {"name": "Widget"}, "quantity": 2, "total_price": "49.98"}
  ]
}
```

### Filter Orders by Status

```bash
# Pending orders
curl "http://localhost:3000/orders?filter={\"status\":\"pending\"}"

# Customer's orders
curl "http://localhost:3000/orders?filter={\"customer_id\":\"{id}\"}"
```

### Update Order Status

```bash
curl -X PUT http://localhost:3000/orders/{id} \
  -H "Content-Type: application/json" \
  -d '{"status": "shipped"}'
```
