use std::{cmp, collections::HashMap, hash::Hash, ops::Index};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

pub const MAX_RECIPE_VALUE: i32 = i32::MAX - 10000;

pub type ItemId = usize;
pub const COBBLESTONE_ID: ItemId = 0;
pub const GRAVEL_ID: ItemId = 1;
pub const SAND_ID: ItemId = 2;
pub const GLASS_ID: ItemId = 3;
pub const DIAMOND_ID: ItemId = 4;

const ITEM_NAMES: [&str; 5] = ["Cobblestone", "Gravel", "Sand", "Glass", "Diamond"];

pub fn item_name(item_id: ItemId) -> &'static str {
    ITEM_NAMES.get(item_id).copied().unwrap_or("Unknown")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    pub items: HashMap<ItemId, usize>,
}

impl ItemSet {
    pub fn new(items: Vec<(ItemId, usize)>) -> Self {
        let mut item_set = Self { items: HashMap::new() };
        for (item_id, count) in items {
            item_set.add(item_id, count);
        }
        item_set
    }

    pub fn add(&mut self, item_id: ItemId, count: usize) {
        *self.items.entry(item_id).or_insert(0) += count;
    }
}

impl Index<ItemId> for ItemSet {
    type Output = usize;

    fn index(&self, item_id: ItemId) -> &Self::Output {
        self.items.get(&item_id).unwrap_or(&0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecipePriorityKey(pub Vec<isize>);

impl cmp::PartialOrd for RecipePriorityKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RecipePriorityKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            match a.cmp(b) {
                cmp::Ordering::Equal => {}
                non_equal => return non_equal,
            }
        }
        self.0.len().cmp(&other.0.len())
    }
}

impl RecipePriorityKey {
    pub fn add_recipe(&mut self, recipe: &Recipe) {
        self.0.push(recipe.base_priority);
    }
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub unique_id: usize,
    pub input: ItemSet,
    pub output: ItemSet,
    pub base_priority: isize,
    pub effective_priority: Option<isize>,
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.unique_id == other.unique_id
    }
}

impl Eq for Recipe {}

impl Hash for Recipe {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_id.hash(state);
    }
}

impl Recipe {
    fn compute_id(&self) -> usize {
        let input_items_vec = self
            .input
            .items
            .iter()
            .map(|(&item_id, &count)| (item_id, count))
            .collect::<Vec<_>>();
        let output_items_vec = self
            .output
            .items
            .iter()
            .map(|(&item_id, &count)| (item_id, count))
            .collect::<Vec<_>>();

        let mut hasher = DefaultHasher::new();
        for (item_id, count) in input_items_vec {
            hasher.write_usize(item_id);
            hasher.write_usize(count);
        }
        for (item_id, count) in output_items_vec {
            hasher.write_usize(item_id);
            hasher.write_usize(count);
        }
        hasher.write_isize(self.base_priority);
        hasher.finish() as usize
    }

    pub fn new_single(input: ItemId, input_count: i32, output: ItemId, output_count: i32, priority: isize) -> Self {
        let mut recipe = Self {
            input: ItemSet::new(vec![(input, input_count as usize)]),
            output: ItemSet::new(vec![(output, output_count as usize)]),
            base_priority: priority,
            effective_priority: None,
            unique_id: 0,
        };
        recipe.unique_id = recipe.compute_id();
        recipe
    }

    pub fn new(input: Vec<(ItemId, usize)>, output: Vec<(ItemId, usize)>, priority: isize) -> Self {
        let mut recipe = Self {
            input: ItemSet::new(input),
            output: ItemSet::new(output),
            base_priority: priority,
            effective_priority: None,
            unique_id: 0,
        };
        recipe.unique_id = recipe.compute_id();
        recipe
    }

    pub fn name(&self) -> String {
        let input_str = self
            .input
            .items
            .iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        let output_str = self
            .output
            .items
            .iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        format!("{} -> {}", input_str, output_str)
    }
}

pub struct CraftingSolution {
    pub recipe_values: HashMap<Recipe, f64>,
    pub final_inventory_values: HashMap<ItemId, f64>,
    pub relevant_item_ids: Vec<ItemId>,
}

impl CraftingSolution {
    pub fn recipe_value(&self, recipe: &Recipe) -> f64 {
        self.recipe_values.get(recipe).copied().unwrap_or(0.0)
    }

    pub fn final_inventory(&self, item_id: ItemId) -> f64 {
        self.final_inventory_values.get(&item_id).copied().unwrap_or(0.0)
    }
}
