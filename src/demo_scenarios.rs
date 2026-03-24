use crate::model::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, Recipe, SAND_ID,
    STRESS_ITEM_BASE_ID, STRESS_ITEM_COUNT,
};

pub type ScenarioData = (Vec<Recipe>, ItemSet, ItemSet);

pub fn build_demo_scenario() -> ScenarioData {
    // Returns a basic demo scenario
    let recipes = vec![
        Recipe::from_single_transform(COBBLESTONE_ID, 1, GRAVEL_ID, 1, 0),
        Recipe::from_single_transform(GRAVEL_ID, 2, SAND_ID, 1, 10),
        Recipe::from_transform(vec![(SAND_ID, 1), (COBBLESTONE_ID, 1)], vec![(GLASS_ID, 2)], 10),
        Recipe::from_single_transform(COBBLESTONE_ID, 10, GLASS_ID, 9, 5),
        Recipe::from_transform(
            vec![(COBBLESTONE_ID, 1)],
            vec![(COBBLESTONE_ID, 2), (DIAMOND_ID, 1)],
            -100000,
        ),
    ];
    let starting_items = ItemSet::from_item_counts(vec![(COBBLESTONE_ID, 5)]);
    let target = ItemSet::from_item_counts(vec![(GLASS_ID, 11)]);

    crate::debugln!(
        "[debug] build_demo_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}

fn stress_item_id(index: usize) -> usize {
    STRESS_ITEM_BASE_ID + (index % STRESS_ITEM_COUNT)
}

pub fn build_stress_scenario() -> ScenarioData {
    // Small stress test of exponentials and cycles. Has to go around an exponential (2^20) loop 50 times
    let mut recipes = Vec::with_capacity(STRESS_ITEM_COUNT + 1);

    for index in 0..(STRESS_ITEM_COUNT - 1) {
        recipes.push(Recipe::from_single_transform(
            stress_item_id(index),
            2,
            stress_item_id(index + 1),
            1,
            0,
        ));
    }

    recipes.push(Recipe::from_single_transform(
        stress_item_id(STRESS_ITEM_COUNT - 1),
        1,
        stress_item_id(0),
        (1 << 19) - 1,
        0,
    ));

    let starting_items = ItemSet::from_item_counts(vec![(stress_item_id(0), 1 << 19)]);
    let target = ItemSet::from_item_counts(vec![(stress_item_id(0), (1 << 19) + 50)]);

    crate::debugln!(
        "[debug] build_stress_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}