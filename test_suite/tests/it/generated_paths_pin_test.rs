//! Every `crudcrate::` path that generated code names, in two tiers.
//!
//! Tier one is emitted by `quote!` blocks in crudcrate-derive. Tier two is
//! emitted by the `#[macro_export]` handler macros and expands in the
//! downstream crate, so the crudcrate lib build never resolves it. Moving a
//! runtime item is free; renaming one of these paths breaks users while
//! `cargo build -p crudcrate` stays green. This file makes that a workspace
//! build failure instead.

#![allow(unused_imports, dead_code)]

// --- Tier one: crudcrate-derive quote! blocks ---
use crudcrate::ApiError;
use crudcrate::CRUDResource;
use crudcrate::EntityToModels;
use crudcrate::JoinedColumnDef;
use crudcrate::JoinedFilter;
use crudcrate::PrimaryKeyType;
use crudcrate::ScopeCondition;
use crudcrate::ScopeFilterable;
use crudcrate::SecurityProfile;
use crudcrate::ToCreateModel;
use crudcrate::ToListModel;
use crudcrate::ToUpdateModel;
use crudcrate::build_comparison_expr;
use crudcrate::build_filter_expr;
use crudcrate::crud_handlers;
use crudcrate::impls;
use crudcrate::serde_with::rust::double_option;
use crudcrate::table_column_ref;
use crudcrate::tracing::warn;
use crudcrate::traits::CRUDResource as _;
use crudcrate::traits::MergeIntoActiveModel;
use crudcrate::validation::__auto::{Probe, ValidatableFallback};

// --- Tier two: crud_handlers_impl! and generate_crud_router! ---
use crudcrate::BatchOptions;
use crudcrate::BatchResult;
use crudcrate::SortConfig::{Column, Joined};
use crudcrate::apply_filters_with_joins;
use crudcrate::crud_handlers_impl;
use crudcrate::filter::{apply_filters, parse_pagination};
use crudcrate::models::FilterOptions;
use crudcrate::pagination::calculate_content_range;
use crudcrate::parse_sorting_with_joins;
use crudcrate::profile::resolve;
use crudcrate::sort::parse_sorting;

fn api_error_constructors() {
    let _ = ApiError::not_found("", None);
    let _ = ApiError::bad_request("");
    let _ = ApiError::forbidden("");
    let _ = ApiError::internal("", None);
    let _ = ApiError::database;
    let _: fn(sea_orm::DbErr) -> ApiError = ApiError::from;
}

fn security_profile_constructors() {
    let _ = SecurityProfile::secure;
    let _ = SecurityProfile::react_admin;
    let _ = SecurityProfile::legacy;
}

fn batch_result_new<T>() {
    let _ = BatchResult::<T>::new;
}

fn scope_filterable_methods<S: ScopeFilterable>() {
    let _ = S::is_scope_visible;
    let _ = S::scope_condition;
}

fn crud_operations_methods<O: crudcrate::CRUDOperations>() {
    let _ = O::create;
    let _ = O::create_many;
    let _ = O::update;
    let _ = O::update_many;
    let _ = O::delete;
    let _ = O::delete_many;
    let _ = O::get_one;
    let _ = O::get_all;
    let _ = O::before_get_one;
    let _ = O::before_get_all;
    let _ = O::after_get_one;
    let _ = O::after_get_all;
}

fn primary_key_type<R: CRUDResource>(_: PrimaryKeyType<R>) {}

#[test]
fn generated_paths_resolve() {}
