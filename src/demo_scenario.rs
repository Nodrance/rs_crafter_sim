use crate::crafting_domain::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, Recipe, SAND_ID,
};

pub type ScenarioData = (Vec<Recipe>, ItemSet, ItemSet);

pub fn build_demo_scenario() -> ScenarioData {
    // Defines and returns the full default demo scenario: recipes, starting inventory, and target.
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
    let starting_items = ItemSet::from_item_counts(vec![(COBBLESTONE_ID, 0)]);
    let target = ItemSet::from_item_counts(vec![(GLASS_ID, 11)]);

    (recipes, starting_items, target)
}

const STRESS_ITEM_BASE_ID: usize = 100;
const STRESS_ITEM_COUNT: usize = 20;

fn stress_item_id(index: usize) -> usize {
    STRESS_ITEM_BASE_ID + index
}

pub fn build_stress_scenario() -> ScenarioData {
    // Defines and returns the full stress scenario: recipes, starting inventory, and target.
    // Uses item IDs 100..119 to avoid overlapping the default demo scenario IDs.
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

    (recipes, starting_items, target)
}
