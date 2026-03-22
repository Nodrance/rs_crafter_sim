use crate::domain::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, Recipe, SAND_ID,
};

pub fn build_demo_recipes() -> Vec<Recipe> {
    // Defines the example recipe graph used by this executable.
    // Includes both normal conversions and a high-priority looping/bonus recipe for stress testing.
    vec![
        Recipe::from_single_transform(COBBLESTONE_ID, 1, GRAVEL_ID, 1, 0),
        Recipe::from_single_transform(GRAVEL_ID, 2, SAND_ID, 1, 10),
        Recipe::from_transform(vec![(SAND_ID, 1), (COBBLESTONE_ID, 1)], vec![(GLASS_ID, 2)], 10),
        Recipe::from_single_transform(COBBLESTONE_ID, 10, GLASS_ID, 9, 5),
        Recipe::from_transform(
            vec![(COBBLESTONE_ID, 1)],
            vec![(COBBLESTONE_ID, 2), (DIAMOND_ID, 1)],
            -100000,
        ),
    ]
}

pub fn build_demo_starting_items() -> ItemSet {
    // Provides the initial inventory state for the demo run.
    // Uses zero cobblestone to force the solver to reason about feasibility and loops.
    ItemSet::from_item_counts(vec![(COBBLESTONE_ID, 0)])
}

pub fn build_demo_target_items() -> ItemSet {
    // Specifies the desired output inventory that solver constraints must satisfy.
    // Current target asks for at least 11 units of glass.
    ItemSet::from_item_counts(vec![(GLASS_ID, 11)])
}
