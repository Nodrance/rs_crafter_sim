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

pub fn item_display_name(item_id: ItemId) -> &'static str {
    // Returns a stable, human-readable item label for logging and UI output.
    // Falls back to "Unknown" if an out-of-range item id is passed.
    ITEM_NAMES.get(item_id).copied().unwrap_or("Unknown")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    pub items: HashMap<ItemId, usize>,
}

impl ItemSet {
    pub fn from_item_counts(items: Vec<(ItemId, usize)>) -> Self {
        // Constructs an ItemSet from a list of (item_id, count) pairs.
        // Duplicate item ids are merged so the internal map always stores total counts per item.
        let mut item_set = Self { items: HashMap::new() };
        for (item_id, count) in items {
            item_set.add_count(item_id, count);
        }
        item_set
    }

    pub fn add_count(&mut self, item_id: ItemId, count: usize) {
        // Increments the quantity tracked for a single item id.
        // This method is the central mutation path for inventory additions.
        *self.items.entry(item_id).or_insert(0) += count;
    }
}

impl Index<ItemId> for ItemSet {
    type Output = usize;

    fn index(&self, item_id: ItemId) -> &Self::Output {
        // Provides read access with zero-default semantics.
        // Missing keys behave as count 0 so callers can index directly without pre-checking.
        self.items.get(&item_id).unwrap_or(&0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecipePriorityKey(pub Vec<isize>);

impl cmp::PartialOrd for RecipePriorityKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        // Delegates to total ordering so this type can be used in sorted structures.
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RecipePriorityKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // Compares priority vectors lexicographically to prefer lower (better) route keys.
        // If common prefixes are equal, shorter vectors are considered smaller.
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
        // Extends the route key by appending the next recipe's base priority.
        // This encodes traversal path quality during recipe relevance expansion.
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
        // Recipes are considered equal when their computed unique ids match.
        // This keeps comparisons cheap and consistent with Hash implementation.
        self.unique_id == other.unique_id
    }
}

impl Eq for Recipe {}

impl Hash for Recipe {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hashes only the stable unique id so HashMap/HashSet behavior aligns with Eq.
        self.unique_id.hash(state);
    }
}

impl Recipe {
    fn compute_unique_id_hash(&self) -> usize {
        // Builds a deterministic hash from inputs, outputs, and base priority.
        // The resulting id is used for stable identity across cloning and lookups.
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
        // Creates a recipe with one input item type and one output item type.
        // This is a convenience constructor for common simple transforms.
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
        // Creates a recipe from arbitrary input/output item vectors.
        // This supports multi-input and multi-output recipes.
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
        // Returns a human-readable recipe description in "inputs -> outputs" format.
        // Used in logs and LP variable names so output remains understandable.
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
    pub recipe_values: HashMap<Recipe, f64>,
    pub final_inventory_values: HashMap<ItemId, f64>,
    pub relevant_item_ids: Vec<ItemId>,
}

impl CraftingSolution {
    pub fn recipe_usage_count(&self, recipe: &Recipe) -> f64 {
        // Reads solved usage count for a recipe, defaulting to 0.0 when absent.
        // This makes caller code concise when iterating over full recipe lists.
        self.recipe_values.get(recipe).copied().unwrap_or(0.0)
    }

    pub fn final_inventory_count(&self, item_id: ItemId) -> f64 {
        // Reads solved ending inventory for an item, defaulting to 0.0 when absent.
        // Missing entries are treated as zero to simplify report generation.
        self.final_inventory_values.get(&item_id).copied().unwrap_or(0.0)
    }
}
