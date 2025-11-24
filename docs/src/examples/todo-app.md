# Todo Application Example

A practical todo app with filtering, sorting, and search.

## Entity Definition

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(15))")]
pub enum Priority {
    #[sea_orm(string_value = "low")]
    Low,
    #[sea_orm(string_value = "medium")]
    Medium,
    #[sea_orm(string_value = "high")]
    High,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "todos")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    #[crudcrate(fulltext)]
    pub description: Option<String>,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(filterable, sortable)]
    pub priority: Priority,

    #[crudcrate(filterable, sortable)]
    pub due_date: Option<DateTimeUtc>,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

## Usage Examples

### Create Todos

```bash
# High priority task
curl -X POST http://localhost:3000/todos \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Review pull request",
    "description": "Check the new authentication feature",
    "completed": false,
    "priority": "high"
  }'

# Medium priority with due date
curl -X POST http://localhost:3000/todos \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Update documentation",
    "completed": false,
    "priority": "medium",
    "due_date": "2024-12-31T23:59:59Z"
  }'
```

### Filter by Status

```bash
# Incomplete tasks
curl "http://localhost:3000/todos?filter={\"completed\":false}"

# Completed tasks
curl "http://localhost:3000/todos?filter={\"completed\":true}"
```

### Filter by Priority

```bash
# High priority only
curl "http://localhost:3000/todos?filter={\"priority\":\"high\"}"

# High and medium
curl "http://localhost:3000/todos?priority_in=high,medium"
```

### Sort by Due Date

```bash
# Earliest due first
curl "http://localhost:3000/todos?sort=[\"due_date\",\"ASC\"]"

# Latest due first
curl "http://localhost:3000/todos?sort=[\"due_date\",\"DESC\"]"
```

### Search

```bash
# Search title and description
curl "http://localhost:3000/todos?q=authentication"
```

### Combined Queries

```bash
# Incomplete high-priority, sorted by due date
curl "http://localhost:3000/todos?filter={\"completed\":false,\"priority\":\"high\"}&sort=[\"due_date\",\"ASC\"]"
```

### Mark Complete

```bash
curl -X PUT http://localhost:3000/todos/{id} \
  -H "Content-Type: application/json" \
  -d '{"completed": true}'
```

## React Admin Integration

```javascript
// dataProvider.js
import { fetchUtils } from 'react-admin';

const apiUrl = 'http://localhost:3000';

export const dataProvider = {
  getList: (resource, params) => {
    const { page, perPage } = params.pagination;
    const { field, order } = params.sort;
    const range = [(page - 1) * perPage, page * perPage - 1];

    const query = {
      sort: JSON.stringify([field, order]),
      range: JSON.stringify(range),
      filter: JSON.stringify(params.filter),
    };

    const url = `${apiUrl}/${resource}?${fetchUtils.queryParameters(query)}`;

    return fetchUtils.fetchJson(url).then(({ headers, json }) => {
      const contentRange = headers.get('Content-Range');
      const total = parseInt(contentRange.split('/').pop(), 10);
      return { data: json, total };
    });
  },
  // ... other methods
};
```
