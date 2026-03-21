use std::{cmp, collections::HashMap, hash::Hash, ops::Index};
use good_lp::{Expression, ProblemVariables, Solution, SolverModel, constraint, default_solver, variable};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

type ItemId = usize;
const COBBLESTONE_ID: ItemId = 0;
const GRAVEL_ID: ItemId = 1;
const SAND_ID: ItemId = 2;
const GLASS_ID: ItemId = 3;
const DIAMOND_ID: ItemId = 4;

const ITEM_NAMES: [&str; 5] = ["Cobblestone", "Gravel", "Sand", "Glass", "Diamond"];
fn item_name(item_id: ItemId) -> &'static str {
    ITEM_NAMES.get(item_id).copied().unwrap_or("Unknown")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ItemSet{
    items: HashMap<ItemId, usize>,
}

impl ItemSet {
    fn new(items: Vec<(ItemId, usize)>) -> Self {
        let mut item_set = Self { items: HashMap::new() };
        for (item_id, count) in items {
            item_set.add(item_id, count);
        }
        item_set
    }

    fn add(&mut self, item_id: ItemId, count: usize) {
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
struct RecipePriorityKey(Vec<isize>);
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
    fn add_recipe(&mut self, recipe: &Recipe) {
        self.0.push(recipe.base_priority);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Recipe{
    unique_id: usize,
    input: ItemSet,
    output: ItemSet,
    base_priority: isize,
    effective_priority: Option<isize>,
}
impl Hash for Recipe {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_id.hash(state);
    }
}
impl Recipe {
    fn compute_id(&self) -> usize {
        let input_items_vec = self.input.items.iter().map(|(&item_id, &count)| (item_id, count)).collect::<Vec<_>>();
        let output_items_vec = self.output.items.iter().map(|(&item_id, &count)| (item_id, count)).collect::<Vec<_>>();
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

    fn new_single(input: ItemId, input_count: i32, output: ItemId, output_count: i32, priority: isize) -> Self {
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

    fn new(input: Vec<(ItemId, usize)>, output: Vec<(ItemId, usize)>, priority: isize) -> Self {
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

    fn name(&self) -> String {
        let input_str = self.input.items.iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        let output_str = self.output.items.iter()
            .map(|(&item_id, &count)| format!("{} x{}", item_name(item_id), count))
            .collect::<Vec<_>>()
            .join(" + ");
        format!("{} -> {}", input_str, output_str)
    }
}

struct CraftingSolution {
    recipe_values: HashMap<Recipe, f64>,
    final_inventory_values: HashMap<ItemId, f64>,
    relevant_item_ids: Vec<ItemId>,
}
impl CraftingSolution {
    fn recipe_value(&self, recipe: &Recipe) -> f64 {
        self.recipe_values.get(recipe).copied().unwrap_or(0.0)
    }

    fn final_inventory(&self, item_id: ItemId) -> f64 {
        self.final_inventory_values.get(&item_id).copied().unwrap_or(0.0)
    }
}

fn sort_and_prune_recipes(recipes: Vec<Recipe>, target: &ItemSet) -> (Vec<Recipe>, std::collections::HashSet<ItemId>) {
    // Each item inherits the best priority key of any recipe that produces it
    // Each recipe inherits the priority key of its best output item type, plus its own base priority
    // This continues until it stabilizes (because loops are possible but will never result in a better key)
    // Then we return only recipes and item types that were given a priority key, because those are the only ones that could possibly be relevant to crafting the target item type
    // We also set each recipe's effective priority to an isize that represents its position in the final sorted order, so we can easily lock priorities later when solving
    // We also make items "relevant" if they are produced by a relevant recipe, to make sure we keep byproducts
    // Depth first search to ensure we always reach a given item/recipe in the lowest priority way
    let mut best_item_priorities: HashMap<ItemId, RecipePriorityKey> = HashMap::new();
    let mut best_recipe_priorities: HashMap<Recipe, RecipePriorityKey> = HashMap::new();
    let mut stack = Vec::new();

    for item_id in target.items.keys() {
        best_item_priorities.insert(*item_id, RecipePriorityKey(Vec::new()));
        stack.push(*item_id);
    }

    while let Some(output_item_id) = stack.pop() {
        let output_priority = best_item_priorities.get(&output_item_id)
            .cloned()
            .expect("Error during recipe sort/prune: Queued items must already have a priority. This should be unreachable.");

        for recipe in recipes.iter() {
            if !recipe.output.items.iter().any(|(&item_id, _)| item_id == output_item_id) {
                // This recipe doesn't produce the item we're currently considering, so skip it
                continue;
            }

            let mut candidate_recipe_priority = output_priority.clone();
            candidate_recipe_priority.add_recipe(recipe);

            // True if this recipe doesn't have a priority yet, or if the candidate priority is better than the current best priority for this recipe
            let should_update_recipe = best_recipe_priorities.get(recipe)
                .map(|current| candidate_recipe_priority < *current)
                .unwrap_or(true);
            if !should_update_recipe {continue;}
            best_recipe_priorities.insert(recipe.clone(), candidate_recipe_priority.clone());

            for input_item_id in recipe.input.items.keys() {
                let should_update_item = best_item_priorities
                    .get(input_item_id)
                    .map(|current| candidate_recipe_priority < *current)
                    .unwrap_or(true);
                if should_update_item {
                    best_item_priorities.insert(*input_item_id, candidate_recipe_priority.clone());
                    stack.push(*input_item_id);
                }
            }
        }
    }

    // Take all the recipes we set a priority for, sort them by priority, and assign effective priority values based on that sorted order
    let mut pruned_recipes = best_recipe_priorities.into_iter().collect::<Vec<_>>();
    pruned_recipes.sort_by_key(|(_, priority)| priority.clone());
    for (index, (recipe, _)) in pruned_recipes.iter_mut().enumerate() {
        recipe.effective_priority = Some(index as isize);
    }
    let recipes = pruned_recipes.into_iter().map(|(recipe, _)| recipe).collect::<Vec<_>>();

    // Take all the item types that are used by those recipes as inputs or outputs, to get the final list of relevant item types (unsorted)
    // This means byproducts are also tracked by the system so they can be shown in the crafting plan
    let mut relevant_item_ids = std::collections::HashSet::new();
    for recipe in &recipes {
        for item_id in recipe.output.items.keys() {
            relevant_item_ids.insert(*item_id);
        }
        for item_id in recipe.input.items.keys() {
            relevant_item_ids.insert(*item_id);
        }
    }
    
    (recipes, relevant_item_ids)
}

fn calculate_solution(recipes: Vec<Recipe>, starting_items: ItemSet, target: ItemSet) -> CraftingSolution {
    let (recipes, relevant_item_ids) = sort_and_prune_recipes(recipes, &target);
    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = problem_variables.add(variable().integer().min(0).name(recipe.name()));
        recipe_to_variable.insert(recipe.clone(), var);
    }

    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();
    for item in &relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in recipes.iter() {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable.get(recipe).unwrap();
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let mut recipe_constraints = Vec::new();
    let mut solution = problem_variables.clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve().expect("It's impossible to craft the target items with the provided recipes and starting items");

    for recipe in &recipes {
        let var = recipe_to_variable.get(recipe).unwrap();
        solution = problem_variables.clone()
        .minimise(*var).using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve().unwrap();
        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
    }

    let mut recipe_values = HashMap::new();
    for recipe in &recipes {
        if let Some(var) = recipe_to_variable.get(recipe) {
            recipe_values.insert(recipe.clone(), solution.value(*var));
        }
    }

    let mut final_inventory_values = HashMap::new();
    for item_id in &relevant_item_ids {
        if let Some(expr) = item_expressions.get(item_id) {
            final_inventory_values.insert(*item_id, expr.eval_with(&solution));
        }
    }

    let mut relevant_item_ids = relevant_item_ids.into_iter().collect::<Vec<_>>();
    relevant_item_ids.sort_unstable();

    CraftingSolution {
        recipe_values,
        final_inventory_values,
        relevant_item_ids,
    }
}

fn main() {
    let recipes = get_recipes();
    let starting_items = get_starting_items();
    let target = get_target();

    let solution = calculate_solution(recipes.clone(), starting_items.clone(), target.clone());
    
    println!("Successfully crafted the target item!");

    println!("\nRecipe usage breakdown:");
    for recipe in &recipes {
        let var_value = solution.recipe_value(recipe);
        if var_value == 0.0 {continue;}
        println!("- {}: {}", recipe.name(), var_value);
    }

    println!("\nFinal inventory:");
    for item in &solution.relevant_item_ids {
        let final_val = solution.final_inventory(*item);
        if final_val == 0.0 {continue;}
        println!("- {}: {}", item_name(*item), final_val);
    }
}

// ##########
// EDIT THESE
// ##########

fn get_recipes() -> Vec<Recipe> {
    vec![
        // One cobblestone into one gravel, priority 0
        Recipe::new_single(
            COBBLESTONE_ID, 1, 
            GRAVEL_ID, 1,
            0),
        Recipe::new_single(
            GRAVEL_ID, 2,
            SAND_ID, 1,
            10),
        // one sand and one cobbleston into 2 glass, priority 10
        Recipe::new(
            vec![(SAND_ID, 1), (COBBLESTONE_ID, 1)],
            vec![(GLASS_ID, 2)],
            10),
        Recipe::new_single(
            COBBLESTONE_ID, 10,
            GLASS_ID, 9,
            5),
        // 1 cobblestone into 2 cobblestone and a diamond, -100000 priority
        Recipe::new(
            vec![(COBBLESTONE_ID, 1)],
            vec![(COBBLESTONE_ID, 2), (DIAMOND_ID, 1)],
            -100000),
    ]
}

fn get_starting_items() -> ItemSet {
    ItemSet::new(vec![
        (COBBLESTONE_ID, 1)
    ])
}

fn get_target() -> ItemSet {
    ItemSet::new(vec![(GLASS_ID, 11)])
}