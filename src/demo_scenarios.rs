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
    // Small stress test of exponentials and cycles.
    // Uses a 20-item loop so one full cycle consumes 2^20 of Alpha and returns 2^20 + 1 Alpha.
    const STRESS_LOOP_ITEM_COUNT: usize = 20;
    const {assert!(STRESS_LOOP_ITEM_COUNT <= STRESS_ITEM_COUNT*2);}

    let mut recipes = Vec::with_capacity(STRESS_LOOP_ITEM_COUNT);

    for index in 0..(STRESS_LOOP_ITEM_COUNT - 1) {
        recipes.push(Recipe::from_single_transform(
            stress_item_id(index),
            2,
            stress_item_id(index + 1),
            1,
            0,
        ));
    }

    recipes.push(Recipe::from_single_transform(
        stress_item_id(STRESS_LOOP_ITEM_COUNT - 1),
        2,
        stress_item_id(0),
        (1 << STRESS_LOOP_ITEM_COUNT) + 1,
        0,
    ));

    let starting_items = ItemSet::from_item_counts(vec![(stress_item_id(0), 1 << STRESS_LOOP_ITEM_COUNT)]);
    let target = ItemSet::from_item_counts(vec![(stress_item_id(0), (1 << STRESS_LOOP_ITEM_COUNT) + 32)]);

    crate::debugln!(
        "[debug] build_stress_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}

pub fn build_sat_scenario() -> ScenarioData {
    // A small scenario that encodes SAT, specifically (a∨¬b∨¬d)∧(¬a∨b∨¬c)∧(b∨¬c∨d)
    let a = STRESS_ITEM_BASE_ID;
    let b = STRESS_ITEM_BASE_ID + 1;
    let c = STRESS_ITEM_BASE_ID + 2;
    let d = STRESS_ITEM_BASE_ID + 3;

    let a_true = a+4;
    let b_true = b+4;
    let c_true = c+4;
    let d_true = d+4;
    let a_false = a+8;
    let b_false = b+8;
    let c_false = c+8;
    let d_false = d+8;

    let term_1 = STRESS_ITEM_BASE_ID + 12;
    let term_2 = STRESS_ITEM_BASE_ID + 13;
    let term_3 = STRESS_ITEM_BASE_ID + 14;

    let starting_items = ItemSet::from_item_counts(vec![(a, 1), (b, 1), (c, 1), (d, 1)]);

    let recipes = vec![
        // Encode the choice of true/false for each variable
        Recipe::from_single_transform(a, 1, a_true, 100, 0),
        Recipe::from_single_transform(a, 1, a_false, 100, 0),
        Recipe::from_single_transform(b, 1, b_true, 100, 0),
        Recipe::from_single_transform(b, 1, b_false, 100, 0),
        Recipe::from_single_transform(c, 1, c_true, 100, 0),
        Recipe::from_single_transform(c, 1, c_false, 100, 0),
        Recipe::from_single_transform(d, 1, d_true, 100, 0),
        Recipe::from_single_transform(d, 1, d_false, 100, 0),

        // Encode the clauses
        Recipe::from_single_transform(a_true, 1, term_1, 1, -100000),
        Recipe::from_single_transform(b_false, 1, term_1, 1, -100000),
        Recipe::from_single_transform(d_false, 1, term_1, 1, -100000),

        Recipe::from_single_transform(a_false, 1, term_2, 1, -100000),
        Recipe::from_single_transform(b_true, 1, term_2, 1, -100000),
        Recipe::from_single_transform(c_false, 1, term_2, 1, -100000),

        Recipe::from_single_transform(b_true, 1, term_3, 1, -100000),
        Recipe::from_single_transform(c_false, 1, term_3, 1, -100000),
        Recipe::from_single_transform(d_true, 1, term_3, 1, -100000),

        // Encode the target as needing all three terms
        Recipe::from_transform(vec![(term_1, 1), (term_2, 1), (term_3, 1)], vec![(STRESS_ITEM_BASE_ID + 23, 1)], 100000),
    ];
    let target = ItemSet::from_item_counts(vec![(STRESS_ITEM_BASE_ID + 23, 1)]);
    (recipes, starting_items, target)
}
