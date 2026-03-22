use crate::domain::{
    COBBLESTONE_ID, DIAMOND_ID, GLASS_ID, GRAVEL_ID, ItemSet, Recipe, SAND_ID,
};

pub fn get_recipes() -> Vec<Recipe> {
    vec![
        Recipe::new_single(COBBLESTONE_ID, 1, GRAVEL_ID, 1, 0),
        Recipe::new_single(GRAVEL_ID, 2, SAND_ID, 1, 10),
        Recipe::new(vec![(SAND_ID, 1), (COBBLESTONE_ID, 1)], vec![(GLASS_ID, 2)], 10),
        Recipe::new_single(COBBLESTONE_ID, 10, GLASS_ID, 9, 5),
        Recipe::new(
            vec![(COBBLESTONE_ID, 1)],
            vec![(COBBLESTONE_ID, 2), (DIAMOND_ID, 1)],
            -100000,
        ),
    ]
}

pub fn get_starting_items() -> ItemSet {
    ItemSet::new(vec![(COBBLESTONE_ID, 0)])
}

pub fn get_target() -> ItemSet {
    ItemSet::new(vec![(GLASS_ID, 11)])
}
