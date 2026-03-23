use crate::crafting_domain::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, Recipe, SAND_ID, STRESS_ITEM_BASE_ID,
    STRESS_ITEM_COUNT,
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
    // Defines and returns a basic stress scenario with a long compression/decompression cycle.
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

pub fn build_loop_stress_scenario() -> ScenarioData {
    // Defines and returns a large stress cycle scenario with many overlapping loops,
    // asymmetric conversions, and mixed multi-input/multi-output branches.
    let s = stress_item_id;

    let recipes = vec![
        Recipe::from_transform(vec![(s(0), 3), (s(4), 1)], vec![(s(1), 5), (s(7), 1)], 2),
        Recipe::from_transform(vec![(s(1), 2), (s(8), 1)], vec![(s(2), 3), (s(11), 2)], -1),
        Recipe::from_transform(vec![(s(2), 5)], vec![(s(3), 2), (s(9), 1)], 4),
        Recipe::from_transform(vec![(s(3), 3), (s(6), 2)], vec![(s(4), 4), (s(12), 1)], 0),
        Recipe::from_transform(vec![(s(4), 6)], vec![(s(5), 3), (s(0), 1)], -3),
        Recipe::from_transform(vec![(s(5), 4), (s(10), 1)], vec![(s(6), 2), (s(13), 2)], 6),
        Recipe::from_transform(vec![(s(6), 5), (s(2), 1)], vec![(s(7), 4)], -2),
        Recipe::from_transform(vec![(s(7), 3), (s(14), 1)], vec![(s(8), 6), (s(15), 1)], 3),
        Recipe::from_transform(vec![(s(8), 4), (s(1), 2)], vec![(s(9), 5), (s(16), 1)], 7),
        Recipe::from_transform(vec![(s(9), 7)], vec![(s(10), 2), (s(5), 3)], -5),
        Recipe::from_transform(vec![(s(10), 3), (s(12), 1)], vec![(s(11), 4), (s(17), 2)], 1),
        Recipe::from_transform(vec![(s(11), 6), (s(3), 1)], vec![(s(12), 5)], 5),
        Recipe::from_transform(vec![(s(12), 2), (s(18), 2)], vec![(s(13), 7), (s(4), 1)], -4),
        Recipe::from_transform(vec![(s(13), 8)], vec![(s(14), 3), (s(19), 2)], 8),
        Recipe::from_transform(vec![(s(14), 2), (s(6), 3)], vec![(s(15), 5), (s(20), 1)], -6),
        Recipe::from_transform(vec![(s(15), 5), (s(9), 1)], vec![(s(16), 4), (s(2), 2)], 2),
        Recipe::from_transform(vec![(s(16), 3), (s(21), 1)], vec![(s(17), 6)], 0),
        Recipe::from_transform(vec![(s(17), 7)], vec![(s(18), 2), (s(10), 3)], 9),
        Recipe::from_transform(vec![(s(18), 4), (s(5), 2)], vec![(s(19), 5), (s(22), 1)], -7),
        Recipe::from_transform(vec![(s(19), 6), (s(11), 1)], vec![(s(20), 4), (s(8), 2)], 4),
        Recipe::from_transform(vec![(s(20), 3), (s(23), 1)], vec![(s(21), 7), (s(12), 1)], -2),
        Recipe::from_transform(vec![(s(21), 5), (s(0), 2)], vec![(s(22), 6), (s(14), 1)], 6),
        Recipe::from_transform(vec![(s(22), 4), (s(13), 1)], vec![(s(23), 3), (s(16), 2)], -1),
        Recipe::from_transform(vec![(s(23), 2), (s(24), 2)], vec![(s(24), 5), (s(1), 1)], 5),
        Recipe::from_transform(vec![(s(24), 9)], vec![(s(0), 4), (s(18), 3)], -8),
        Recipe::from_transform(vec![(s(2), 3), (s(15), 2)], vec![(s(6), 6), (s(22), 1)], 3),
        Recipe::from_transform(vec![(s(7), 1), (s(19), 3)], vec![(s(3), 5), (s(10), 1)], -3),
        Recipe::from_transform(vec![(s(4), 2), (s(17), 2)], vec![(s(9), 4), (s(24), 1)], 1),
        Recipe::from_transform(vec![(s(11), 3), (s(20), 2)], vec![(s(5), 6), (s(23), 1)], -4),
        Recipe::from_transform(vec![(s(16), 2), (s(8), 3)], vec![(s(12), 7), (s(21), 1)], 2),
        Recipe::from_transform(vec![(s(18), 1), (s(14), 4)], vec![(s(2), 5), (s(7), 2)], -2),
        Recipe::from_transform(vec![(s(1), 5), (s(22), 1)], vec![(s(13), 4), (s(19), 2)], 4),
        Recipe::from_transform(vec![(s(6), 2), (s(24), 3)], vec![(s(17), 5), (s(0), 1)], -5),
        Recipe::from_transform(vec![(s(9), 2), (s(3), 2), (s(21), 1)], vec![(s(15), 6)], 7),
        Recipe::from_transform(vec![(s(10), 4), (s(12), 2)], vec![(s(18), 5), (s(4), 2)], -6),
        Recipe::from_transform(vec![(s(23), 3), (s(5), 1)], vec![(s(11), 6), (s(16), 1)], 0),
        Recipe::from_transform(vec![(s(20), 2), (s(2), 2), (s(7), 1)], vec![(s(24), 4), (s(14), 2)], 5),
        Recipe::from_transform(vec![(s(13), 4), (s(0), 1)], vec![(s(8), 5), (s(22), 2)], -1),
        Recipe::from_transform(vec![(s(15), 3), (s(6), 1), (s(19), 1)], vec![(s(1), 7), (s(20), 1)], 3),
        Recipe::from_transform(vec![(s(17), 2), (s(10), 1), (s(4), 1)], vec![(s(23), 5), (s(9), 1)], -4),
        Recipe::from_transform(vec![(s(24), 2), (s(14), 1), (s(11), 2)], vec![(s(3), 6), (s(18), 1)], 6),
        Recipe::from_transform(vec![(s(22), 3), (s(16), 1), (s(5), 2)], vec![(s(12), 8)], -7),
    ];

    let starting_items = ItemSet::from_item_counts(vec![
        (s(0), 850),
        (s(3), 420),
        (s(7), 310),
        (s(12), 215),
        (s(19), 160),
        (s(24), 95),
    ]);

    let target = ItemSet::from_item_counts(vec![(s(0), 1300), (s(18), 420), (s(23), 260)]);

    crate::debugln!(
        "[debug] build_loop_stress_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}
