#!/bin/bash

echo "🧪 Running Generic Joins Test Suite"
echo "These tests WILL FAIL initially - they expose hardcoding issues"
echo "They should pass once we implement fully generic join loading"
echo ""

echo "=================================="
echo "Test 1: Vehicle → Parts (Generic)"
echo "=================================="
cargo test test_generic_join_vehicle_to_parts --features test_mode -- --nocapture

echo ""
echo "================================================="
echo "Test 2: Vehicle → Maintenance Records (Generic)"
echo "================================================="
cargo test test_generic_join_vehicle_to_maintenance_records --features test_mode -- --nocapture

echo ""
echo "======================================="
echo "Test 3: Recursive Joins (Depth=2)"
echo "======================================="
cargo test test_recursive_joins_depth_2 --features test_mode -- --nocapture

echo ""
echo "=========================================="
echo "Test 4: Multiple Entity Types Support"
echo "=========================================="
cargo test test_multiple_entity_types_generic_joins --features test_mode -- --nocapture

echo ""
echo "🎯 Summary:"
echo "- Failing tests expose hardcoding issues that need to be fixed"
echo "- Once generic implementation is complete, all tests should pass"
echo "- This proves we have truly generic, non-hardcoded join loading"