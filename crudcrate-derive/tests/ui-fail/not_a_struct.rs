//! EntityToModels only derives for structs; an enum must produce a clear error
//! rather than a confusing downstream failure.

use crudcrate::EntityToModels;

#[derive(EntityToModels)]
enum NotAStruct {
    A,
    B,
}

fn main() {}
