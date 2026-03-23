use rs_crafter_sim::{
    crafting_domain::{ItemSet, Recipe},
    crafting_solver::compute_required_base_items,
};

fn assert_required_base_items(required: &ItemSet, expected: &[(usize, usize)]) {
    assert_eq!(
        required.items.len(),
        expected.len(),
        "Expected exactly {:?} required base items, but got {:?}",
        expected,
        required.items
    );

    for (item_id, expected_count) in expected {
        assert_eq!(
            required[*item_id],
            *expected_count,
            "Unexpected required count for item {}",
            item_id
        );
    }
}

#[test]
fn compute_required_base_items_reports_single_missing_base_item() {
    let recipes = vec![
        Recipe::from_single_transform(0, 1, 1, 1, 0),
        Recipe::from_single_transform(1, 1, 2, 1, 0),
    ];
    let starting_items = ItemSet::from_item_counts(vec![(0, 1)]);
    let target = ItemSet::from_item_counts(vec![(2, 2)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(0, 1)]);
}

#[test]
fn compute_required_base_items_returns_empty_when_starting_inventory_is_sufficient() {
    let recipes = vec![
        Recipe::from_single_transform(0, 1, 1, 1, 0),
        Recipe::from_single_transform(1, 1, 2, 1, 0),
    ];
    let starting_items = ItemSet::from_item_counts(vec![(0, 2)]);
    let target = ItemSet::from_item_counts(vec![(2, 2)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[]);
}

#[test]
fn compute_required_base_items_reports_multiple_missing_base_items_for_multi_input_recipe() {
    let recipes = vec![Recipe::from_transform(vec![(0, 2), (3, 1)], vec![(2, 1)], 0)];
    let starting_items = ItemSet::from_item_counts(vec![(0, 2), (3, 1)]);
    let target = ItemSet::from_item_counts(vec![(2, 2)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(0, 2), (3, 1)]);
}

#[test]
fn compute_required_base_items_handles_non_producible_target_items_with_relevant_recipe_graph() {
    let recipes = vec![Recipe::from_single_transform(0, 1, 2, 1, 0)];
    let starting_items = ItemSet::from_item_counts(vec![(0, 1), (7, 1)]);
    let target = ItemSet::from_item_counts(vec![(2, 1), (7, 4)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(7, 3)]);
}

#[test]
fn compute_required_base_items_labels_non_producible_target_as_required_input() {
    let recipes = vec![];
    let starting_items = ItemSet::from_item_counts(vec![(7, 1)]);
    let target = ItemSet::from_item_counts(vec![(7, 4)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(7, 3)]);
}

#[test]
fn compute_required_base_items_breaks_cycle_branch_when_target_not_in_cycle() {
    let recipes = vec![
        Recipe::from_single_transform(0, 1, 1, 1, 0),
        Recipe::from_single_transform(1, 1, 0, 1, 0),
        Recipe::from_single_transform(1, 1, 2, 1, 0),
    ];
    let starting_items = ItemSet::from_item_counts(vec![]);
    let target = ItemSet::from_item_counts(vec![(2, 1)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(0, 1)]);
}

#[test]
fn compute_required_base_items_breaks_cycle_branch_when_target_is_in_cycle() {
    let recipes = vec![
        Recipe::from_single_transform(0, 1, 1, 1, 0),
        Recipe::from_single_transform(1, 1, 0, 1, 0),
    ];
    let starting_items = ItemSet::from_item_counts(vec![]);
    let target = ItemSet::from_item_counts(vec![(0, 1)]);

    let required = compute_required_base_items(recipes, starting_items, target);

    assert_required_base_items(&required, &[(1, 1)]);
}
