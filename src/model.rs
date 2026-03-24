// This module defines the core data structures for items, recipes, and crafting solutions.
// The only notable things here are that each item is assumed to have a unique ID
// This means that this system currently only works when every item has one "canonical" ID, and any item with that ID is the same for crafting purposes.
// This rules out for example a recipe like "2 of any plank make sticks", unless you do a translation step to assign all planks the same ID before processing,
// then translate back after a plan is generated

use std::{cmp, collections::HashMap, ops::Index};
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
pub const STRESS_ITEM_BASE_ID: usize = 100;
pub const STRESS_ITEM_COUNT: usize = 25;
const STRESS_ITEM_NAMES: [&str; STRESS_ITEM_COUNT] = [
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota", "Kappa",
    "Lambda", "Mu", "Nu", "Xi", "Omicron", "Pi", "Rho", "Sigma", "Tau", "Upsilon",
    "Phi", "Chi", "Psi", "Omega", "OmegaPrime",
];

// for an demo based on making a klien star from EMC loops
// didn't pan out
pub const KLIEN_ITEM_BASE_ID: usize = 200;
pub const KLIEN_ITEM_COUNT: usize = 21;
const KLIEN_ITEM_NAMES: [&str; KLIEN_ITEM_COUNT] = [
    "EMC",
    "Dirt",
    "Oak Log",
    "Charcoal",
    "Ink Sac",
    "Glow Ink",
    "Lapis",
    "Amethyst",
    "Diamond",
    "Emerald",
    "Klein Star Ein",
    "Klein Star Zwei",
    "Klein Star Drei",
    "Klein Star Vier",
    "Klein Star Sphere",
    "Klein Star Omega",
    "Magnus Star",
    "Colossal Star",
    "Gargantuan Star",
    "Final Star Shard",
    "Final Star",
];

pub fn item_display_name(item_id: ItemId) -> &'static str {
    // Returns a stable, human-readable item label for logs and console output.
    // Falls back to "Unknown" when the caller passes an unmapped id.
    if let Some(name) = ITEM_NAMES.get(item_id) {
        name
    } else if (STRESS_ITEM_BASE_ID..STRESS_ITEM_BASE_ID + STRESS_ITEM_COUNT).contains(&item_id) {
        let index = item_id - STRESS_ITEM_BASE_ID;
        STRESS_ITEM_NAMES.get(index).copied().unwrap_or("Unknown")
    } else if (KLIEN_ITEM_BASE_ID..KLIEN_ITEM_BASE_ID + KLIEN_ITEM_COUNT).contains(&item_id) {
        let index = item_id - KLIEN_ITEM_BASE_ID;
        KLIEN_ITEM_NAMES.get(index).copied().unwrap_or("Unknown")
    } else {
        "Unknown"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    pub items: HashMap<ItemId, usize>,
}

impl ItemSet {
    pub fn from_item_counts(items: Vec<(ItemId, usize)>) -> Self {
        // Builds an ItemSet from (item_id, count) tuples.
        // Duplicate ids are merged so each key stores a single accumulated total.
        let mut item_set = Self { items: HashMap::new() };
        for (item_id, count) in items {
            item_set.add_count(item_id, count);
        }
        item_set
    }

    pub fn add_count(&mut self, item_id: ItemId, count: usize) {
        // Increases the tracked quantity for one item id.
        // Centralizing this update keeps inventory mutations consistent.
        *self.items.entry(item_id).or_insert(0) += count;
    }
}

impl Index<ItemId> for ItemSet {
    type Output = usize;

    fn index(&self, item_id: ItemId) -> &Self::Output {
        // Provides zero-default indexed reads.
        // Missing keys are treated as 0 so callers can index directly without guard logic.
        self.items.get(&item_id).unwrap_or(&0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecipePriorityKey(pub Vec<isize>);

impl cmp::PartialOrd for RecipePriorityKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        // Defers to total ordering so this key can be used in ordered collections.
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RecipePriorityKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // Lexicographically compares route-priority vectors, preferring lower keys.
        // When prefixes match, the shorter vector ranks first.
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
    pub fn append_recipe_priority(&mut self, recipe: &Recipe) {
        // Extends a route key with the next recipe's base priority value.
        // Used while propagating priorities during relevance traversal.
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
        // Recipe equality is based solely on the computed unique id.
        // This keeps Eq consistent with Hash and avoids deep structural comparison.
        self.unique_id == other.unique_id
    }
}

impl Eq for Recipe {}

impl Recipe {
    fn compute_unique_id_hash(&self) -> usize {
        // Computes a deterministic identity hash from inputs, outputs, and base priority.
        // The result provides stable recipe identity across clones and map/set lookups.
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

    pub fn from_single_transform(input: ItemId, input_count: i32, output: ItemId, output_count: i32, priority: isize) -> Self {
        // Constructs a single-input, single-output recipe.
        // Convenience helper for straightforward one-to-one transforms.
        let mut recipe = Self {
            input: ItemSet::from_item_counts(vec![(input, input_count as usize)]),
            output: ItemSet::from_item_counts(vec![(output, output_count as usize)]),
            base_priority: priority,
            effective_priority: None,
            unique_id: 0,
        };
        recipe.unique_id = recipe.compute_unique_id_hash();
        recipe
    }

    pub fn from_transform(input: Vec<(ItemId, usize)>, output: Vec<(ItemId, usize)>, priority: isize) -> Self {
        // Constructs a recipe from arbitrary input and output vectors.
        // Supports multi-input and multi-output transforms.
        let mut recipe = Self {
            input: ItemSet::from_item_counts(input),
            output: ItemSet::from_item_counts(output),
            base_priority: priority,
            effective_priority: None,
            unique_id: 0,
        };
        recipe.unique_id = recipe.compute_unique_id_hash();
        recipe
    }

    pub fn describe(&self) -> String {
        // Builds a human-readable description in "inputs -> outputs" form.
        // Used in diagnostics and as LP variable labels.
        let input_str = self
            .input
            .items
            .iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_display_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        let output_str = self
            .output
            .items
            .iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_display_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        format!("{} -> {}", input_str, output_str)
    }
}

pub struct CraftingSolution {
    pub recipe_values: HashMap<usize, f64>,
    pub final_inventory_values: ItemSet,
    pub relevant_item_ids: Vec<ItemId>,
}

impl CraftingSolution {
    pub fn recipe_usage_count(&self, recipe: &Recipe) -> f64 {
        // Reads solved usage for a recipe, defaulting to 0.0 when absent.
        // This keeps reporting logic simple when iterating complete recipe lists.
        self.recipe_values.get(&recipe.unique_id).copied().unwrap_or(0.0)
    }

    pub fn final_inventory_count(&self, item_id: ItemId) -> f64 {
        // Reads solved ending inventory for an item, defaulting to 0.0 if missing.
        // Missing entries are intentionally interpreted as zero for reporting.
        self.final_inventory_values[item_id] as f64
    }
}
