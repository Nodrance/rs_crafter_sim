use crate::crafting_domain::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, KLIEN_ITEM_BASE_ID, Recipe, SAND_ID,
    STRESS_ITEM_BASE_ID, STRESS_ITEM_COUNT,
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
    // Minimal scenario that reproduces the relaxed base-item panic.
    // - `s(0) <-> s(1)` is a closed conservation loop, so `s(0) >= 1` is impossible from empty start.
    // - `s(3) -> s(2)` makes target item `s(2)` producible, while introducing one non-producible
    //   input item `s(3)` so deficit analysis relaxes only that item.
    // This keeps the panic in `compute_required_base_items(...)` rather than failing earlier.
    let s = stress_item_id;

    let recipes = vec![
        Recipe::from_single_transform(s(0), 1, s(1), 1, 0),
        Recipe::from_single_transform(s(1), 1, s(0), 1, 0),
        Recipe::from_single_transform(s(3), 1, s(2), 1, 0),
    ];

    let starting_items = ItemSet::from_item_counts(vec![]);

    let target = ItemSet::from_item_counts(vec![(s(0), 1), (s(2), 1)]);

    crate::debugln!(
        "[debug] build_loop_stress_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}

pub fn build_klien_star_scenario() -> ScenarioData {
    let k = |index: usize| KLIEN_ITEM_BASE_ID + index;
    let dirt      = k(1);
    let oak_log   = k(2);
    let charcoal  = k(3);
    let ink_sac   = k(4);
    let glow_ink  = k(5);
    let lapis     = k(6);
    let amethyst  = k(7);
    let diamond   = k(8);
    let emerald   = k(9);
    let ein       = k(10);
    let zwei      = k(11);
    let drei      = k(12);
    let vier      = k(13);
    let sphere    = k(14);
    let omega     = k(15);
    let magnus    = k(16);
    let colossal  = k(17);
    let gargantuan = k(18);
    let shard     = k(19);
    let final_star = k(20);

    let recipes = vec![
        // --- Direct transformations ---
        Recipe::from_single_transform(dirt, 1, dirt, 2, 0),
        Recipe::from_single_transform(oak_log, 1, charcoal, 1, 1),
        Recipe::from_single_transform(ink_sac, 1, glow_ink, 1, 2),
        Recipe::from_single_transform(lapis, 1, amethyst, 1, 3),
        Recipe::from_single_transform(diamond, 1, emerald, 1, 4),
        // --- Reverse transformations ---
        Recipe::from_single_transform(charcoal, 1, oak_log, 2, 1),
        Recipe::from_single_transform(glow_ink, 1, ink_sac, 2, 2),
        Recipe::from_single_transform(amethyst, 1, lapis, 2, 3),
        Recipe::from_single_transform(emerald, 1, diamond, 2, 4),
        // --- Klein Star crafting chain ---
        Recipe::from_single_transform(emerald,        2, ein,        1, 0),
        Recipe::from_single_transform(ein,             4, zwei,       1, 0),
        Recipe::from_single_transform(zwei,            4, drei,       1, 0),
        Recipe::from_single_transform(drei,            4, vier,       1, 0),
        Recipe::from_single_transform(vier,            4, sphere,     1, 0),
        Recipe::from_single_transform(sphere,          4, omega,      1, 0),
        Recipe::from_single_transform(omega,           4, magnus,     1, 0),
        Recipe::from_single_transform(magnus,          4, colossal,   1, 0),
        Recipe::from_single_transform(colossal,        4, gargantuan, 1, 0),
        Recipe::from_single_transform(gargantuan,      8, shard,      1, 0),
        Recipe::from_single_transform(shard,       20000, final_star, 1, 0),
    ];

    let starting_items = ItemSet::from_item_counts(vec![(dirt,1)]);

    let target = ItemSet::from_item_counts(vec![(final_star, 1)]);

    crate::debugln!(
        "[debug] build_klien_star_scenario: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    (recipes, starting_items, target)
}
