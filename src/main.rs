const WORKS_AT_1023: i32 = 1023;


use std::{cmp, collections::{HashMap, HashSet}, hash::Hash, ops::Index};
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

#[derive(Debug, Clone)]
struct Recipe{
    unique_id: usize,
    input: ItemSet,
    output: ItemSet,
    base_priority: isize,
    effective_priority: Option<isize>,
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

fn sort_and_prune_relevant_recipes_and_items(recipes: Vec<Recipe>, target: &ItemSet) -> (Vec<Recipe>, HashSet<ItemId>) {
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
    let mut relevant_item_ids = HashSet::new();
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

fn find_items_with_no_recipes(recipes: &[Recipe], relevant_item_ids: &HashSet<ItemId>) -> HashSet<ItemId> {
    relevant_item_ids
        .iter()
        .copied()
        .filter(|item_id| !recipes.iter().any(|recipe| recipe.output[*item_id] > 0))
        .collect()
}

fn filter_highest_priority_recipes_per_item(recipes: &[Recipe]) -> Vec<Recipe> {
    let mut sorted = recipes.to_vec();
    sorted.sort_by_key(|recipe| recipe.effective_priority.unwrap_or(isize::MAX));

    let mut seen_output_items = HashSet::new();
    let mut selected_recipe_ids = HashSet::new();

    for recipe in &sorted {
        let produces_new_item = recipe
            .output
            .items
            .keys()
            .any(|item_id| !seen_output_items.contains(item_id));

        if !produces_new_item {
            continue;
        }

        selected_recipe_ids.insert(recipe.unique_id);
        for item_id in recipe.output.items.keys() {
            seen_output_items.insert(*item_id);
        }
    }

    sorted
        .into_iter()
        .filter(|recipe| selected_recipe_ids.contains(&recipe.unique_id))
        .collect()
}

fn get_required_items(recipes: Vec<Recipe>, starting_items: ItemSet, target: ItemSet) -> ItemSet {
    let (recipes, pruned_relevant_item_ids) = sort_and_prune_relevant_recipes_and_items(recipes, &target);
    let recipes = filter_highest_priority_recipes_per_item(&recipes);

    let mut relevant_item_ids = HashSet::new();
    for item_id in target.items.keys() {
        relevant_item_ids.insert(*item_id);
    }
    for recipe in &recipes {
        for item_id in recipe.input.items.keys() {
            relevant_item_ids.insert(*item_id);
        }
        for item_id in recipe.output.items.keys() {
            relevant_item_ids.insert(*item_id);
        }
    }
    for item_id in &pruned_relevant_item_ids {
        relevant_item_ids.insert(*item_id);
    }

    let items_with_no_recipes = find_items_with_no_recipes(&recipes, &relevant_item_ids);

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
            let var = recipe_to_variable.get(recipe).expect("A recipe is being used in get_required_items that has no variable attached");
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }

        item_expressions.insert(*item, constraint_expr.clone());

        if !items_with_no_recipes.contains(item) {
            item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
        }
    }

    let mut recipe_constraints = Vec::new();
    let mut solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .expect("Could not solve relaxed required-items model");

    for recipe in &recipes {
        let var = recipe_to_variable
            .get(recipe)
            .expect("A recipe is being minimized in get_required_items that has no variable attached");
        solution = problem_variables
            .clone()
            .minimise(*var)
            .using(default_solver)
            .with_all(item_constraints.clone())
            .with_all(recipe_constraints.clone())
            .solve()
            .expect("Could not solve relaxed required-items model while minimizing recipe usage");

        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
    }

    let mut required = ItemSet::new(vec![]);
    for item_id in &items_with_no_recipes {
        let final_inventory = item_expressions
            .get(item_id)
            .expect("Missing expression for non-producible item in get_required_items")
            .eval_with(&solution);
        let needed = (target[*item_id] as f64 - final_inventory).ceil().max(0.0) as usize;
        if needed > 0 {
            required.add(*item_id, needed);
        }
    }

    required
}

fn calculate_solution(recipes: Vec<Recipe>, starting_items: ItemSet, target: ItemSet) -> CraftingSolution {
    println!("Pruning irrelevant recipes and items...");
    let (recipes, relevant_item_ids) = sort_and_prune_relevant_recipes_and_items(recipes, &target);
    println!("Setting up linear program with {} variables and {} constraints...", recipes.len(), relevant_item_ids.len());
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
            let var = recipe_to_variable.get(recipe).expect("A recipe is being used in the calculate solution function that has no variable attached");
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let mut recipe_constraints = Vec::new();
    println!("Setting up initial solution...");
    let mut solution = problem_variables.clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve().expect("It's impossible to craft the target items with the provided recipes and starting items");
    println!("Initial solution found. Locking in recipe usage one by one based on priority...");
    for recipe in &recipes {
        let var = recipe_to_variable.get(recipe).unwrap();
        solution = problem_variables.clone()
        .minimise(*var).using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve().expect("Somehow there's no solution anymore after trying to minimize recipe usage");
        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
        println!("Locked in {} usages for recipe '{}'", var_value, recipe.name());
    }
    println!("All recipe usages locked in. Final solution found.");

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

fn recipe_usage_to_application_plan(
    recipes: &[Recipe],
    recipe_values: &HashMap<Recipe, f64>,
    starting_items: &ItemSet,
) -> Result<Vec<(Recipe, usize)>, String> {
    println!("Converting recipe usage counts into an executable crafting plan...");
    let mut remaining_counts: HashMap<usize, usize> = HashMap::new();
    for recipe in recipes {
        let raw_value = recipe_values.get(recipe).copied().unwrap_or(0.0);
        if raw_value < 0.0 {
            return Err(format!("Negative usage count for recipe '{}'", recipe.name()));
        }

        let rounded = raw_value.round();
        if (raw_value - rounded).abs() > 1e-6 {
            return Err(format!(
                "Non-integer usage count {} for recipe '{}'",
                raw_value,
                recipe.name()
            ));
        }

        let count = rounded as usize;
        if count > 0 {
            remaining_counts.insert(recipe.unique_id, count);
        }
    }

    let mut inventory = starting_items.clone();
    let mut total_remaining: usize = remaining_counts.values().sum();
    let mut plan: Vec<(Recipe, usize)> = Vec::new();

    while total_remaining > 0 {
        let mut made_progress = false;

        for recipe in recipes {
            let remaining = remaining_counts.get(&recipe.unique_id).copied().unwrap_or(0);
            if remaining == 0 {
                continue;
            }

            let mut max_batch = usize::MAX;
            for (item_id, input_count) in &recipe.input.items {
                if *input_count == 0 {
                    continue;
                }
                let available = inventory[*item_id];
                max_batch = max_batch.min(available / *input_count);
            }

            if max_batch == 0 {
                continue;
            }

            let batch = max_batch.min(remaining);
            if batch == 0 {
                continue;
            }

            for (item_id, input_count) in &recipe.input.items {
                let required = input_count * batch;
                let available_entry = inventory.items.entry(*item_id).or_insert(0);
                if *available_entry < required {
                    return Err(format!(
                        "Scheduling bug: recipe '{}' requires {} {} but only {} available",
                        recipe.name(),
                        required,
                        item_name(*item_id),
                        *available_entry
                    ));
                }
                *available_entry -= required;
            }

            for (item_id, output_count) in &recipe.output.items {
                inventory.add(*item_id, output_count * batch);
            }

            remaining_counts.insert(recipe.unique_id, remaining - batch);
            total_remaining -= batch;

            if let Some((last_recipe, last_batch)) = plan.last_mut() {
                if last_recipe.unique_id == recipe.unique_id {
                    *last_batch += batch;
                } else {
                    plan.push((recipe.clone(), batch));
                }
            } else {
                plan.push((recipe.clone(), batch));
            }

            made_progress = true;
        }

        if !made_progress {
            let blocked = recipes
                .iter()
                .filter_map(|recipe| {
                    let remaining = remaining_counts.get(&recipe.unique_id).copied().unwrap_or(0);
                    if remaining > 0 {
                        Some(format!("{} (remaining {})", recipe.name(), remaining))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            return Err(format!(
                "Could not produce a valid execution order with current usage counts. Blocked recipes: {}",
                blocked
            ));
        }
    }

    Ok(plan)
}

fn calculate_max(recipes: Vec<Recipe>, starting_items: ItemSet, target: ItemSet) -> usize {
    let (recipes, relevant_item_ids) = sort_and_prune_relevant_recipes_and_items(recipes, &target);
    let target_item_id = *target
        .items
        .keys()
        .next()
        .expect("Target item set is empty in calculate_max");
    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = problem_variables.add(variable().integer().min(0).max(i32::MAX-WORKS_AT_1023).name(recipe.name()));
        recipe_to_variable.insert(recipe.clone(), var);
    }

    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();
    for item in &relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in recipes.iter() {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable.get(recipe).expect("A recipe is being used in the calculate solution function that has no variable attached");
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let objective = item_expressions
        .get(&target_item_id)
        .expect("A target item is being used in the calculate max function that has no expression attached");
    println!("Solving for maximum number of target items craftable...");
    let solution = problem_variables.clone()
        .maximise(objective.clone())
        .using(default_solver)
        .with_all(item_constraints.clone())
        .solve();
    println!("Solution process complete.");
    if solution.is_err() {
        return 0;
    }

    let result = objective.eval_with(&solution.unwrap()) - starting_items[target_item_id] as f64;
    if result.fract() != 0.0 {
        panic!("Solution is somehow not an integer: {}", result);
    }
    result as usize
}

fn main() {
    let recipes = get_recipes();
    let starting_items = get_starting_items();
    let target = get_target();

    let max = calculate_max(recipes.clone(), starting_items.clone(), target.clone());
    println!("Maximum number of first target item that can be crafted: {}", max);
    if max == 0 {
        println!("No solution found, cannot craft any of the target items with the provided recipes and starting items.");
        let required_items = get_required_items(recipes, starting_items, target);
        if required_items.items.is_empty() {
            println!("No additional base items identified by relaxed solve.");
        } else {
            println!("Required items to add to starting inventory:");
            for (item_id, count) in required_items.items {
                println!("- {}: {}", item_name(item_id), count);
            }
        }
        return;
    }
    let solution = calculate_solution(recipes.clone(), starting_items.clone(), target.clone());
    
    println!("Successfully crafted the target item!");

    println!("\nRecipe usage breakdown:");
    for recipe in &recipes {
        let var_value = solution.recipe_value(recipe);
        if var_value == 0.0 {continue;}
        println!("- {}: {}", recipe.name(), var_value);
    }

    match recipe_usage_to_application_plan(&recipes, &solution.recipe_values, &starting_items) {
        Ok(plan) => {
            println!("\nExecutable recipe plan:");
            for (recipe, count) in plan {
                println!("- Apply '{}' x{}", recipe.name(), count);
            }
        }
        Err(error) => {
            println!("\nCould not build executable recipe plan: {}", error);
        }
    }

    println!("\nFinal inventory:");
    for item in &solution.relevant_item_ids {
        let final_val = solution.final_inventory(*item);
        if final_val == 0.0 {continue;}
        println!("- {}: {}", item_name(*item), final_val);
    }
}

// ⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠰⣮⡫⢮⣔⢄⢠⡀⠀⠀⠀⢆⣾⣥⣾⡞⠏⣂⠁⠒⡛⠠⠤⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⠠⠀⠄⠂⡄
// ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣻⣿⣶⣏⣧⡺⡑⠀⣀⡜⣽⣿⣿⠷⢋⠩⠤⠬⠗⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠐⠠⠁⠂⠡⠀
// ⢀⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠒⠪⠷⡿⣿⣿⣴⣡⢫⡰⣼⡿⡿⣳⠦⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
// ⡌⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣴⣿⣿⣿⣿⣾⡿⠋⠉⠩⡉⣣⣍⢄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠄⠂
// ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⣿⣿⣿⣿⣿⣿⣇⡏⣀⣭⢨⡀⢷⡈⢆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⡐⠡
// ⠤⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠰⣿⣿⣿⣿⣿⣽⣽⣿⣿⣾⣿⡿⠙⠈⣟⡿⡀⠀⠀⠀⠀⠀Edit This⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠁
// ⠀⠀⠀⠉⠁⠒⠀⠤⠀⢀⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⢹⣿⣿⣿⣿⣿⣯⣌⠙⠛⢠⣶⣧⣩⠾⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⠀⠀⠀⡄⢃
// ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸⣿⣿⣿⣿⣿⣿⣿⣿⣋⣙⡢⢄⠭⣽⢩⢟⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠂⠁⠄⡀⠀⠠⢡⠘⡄
// ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣷⣾⣿⣿⣿⣿⣛⠉⠉⠉⠉⢹⣿⣷⢽⠀⡿⣫⢀⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠂⡉⠐⡀⠆⡡⢂⡑⢢
// ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣠⣿⣿⣿⣿⣿⣿⣶⣐⣀⣤⢲⣾⡟⢉⡾⢘⣯⡾⣷⣚⠢⡆⠔⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⠐⡀⠂⠄⡑⢄⠱⣀⠣⢌⡅
// ⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠐⠲⠵⣻⣿⣿⣿⣿⣿⣿⣿⡏⠀⣎⢸⡓⢀⠞⣤⣿⡿⢳⣕⡲⠞⠀⠀⠀⠀⠀⠀⠀⠀⠀⠌⠀⠄⠡⢀⡑⢈⠔⣈⠒⣄⠣⢎⡰
// ⠀⠠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⣴⣾⣿⣿⣿⣿⣿⣿⣿⣿⣷⣦⣇⣼⣤⣴⣿⣿⣿⠇⠀⢟⣏⣁⡚⠂⠀⠀⠀⡀⢀⠂⢀⠀⠡⢈⠐⢂⠄⢣⠘⡄⢣⠜⡌⢦⡑
// ⠈⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠴⠦⠶⢿⣿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣧⠁⠀⡀⠫⣁⡀⣀⠀⠀⢁⠠⠀⠌⡀⢂⡐⠀⠈⠂⠜⣠⠣⠜⣡⠚⡜⡢⠍
// ⠌⠠⢈⠐⡀⠀⠀⠀⠀⠠⠀⠂⠀⢀⢂⡆⡀⡠⣲⣽⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⢹⣿⣿⣟⢟⣯⣬⣭⣾⡿⠏⢢⣷⣈⠀⠄⡁⢆⠘⡠⢐⠡⠂⠀⠈⠤⢣⡙⣤⢋⠔⢡⢚
// ⢌⠰⢀⠢⠐⠡⢀⠀⠄⡁⠂⣀⠔⣱⣎⢁⣫⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡏⠀⡜⣻⣽⣾⣿⣿⣿⡿⠟⠁⣀⢾⡟⢈⢷⠠⡐⢌⠢⡑⢌⡒⡡⠀⠀⢈⣇⠺⡔⡃⢠⢇⢯
// ⢢⠘⡠⢑⠨⡁⢂⠌⣐⠤⣕⣥⣾⡏⠰⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣞⣿⣿⠁⢐⢨⣿⣿⣿⣿⡿⠏⢡⢄⣴⣿⡏⢠⣿⣏⠱⢌⠄⠣⢌⣷⣰⡅⠂⠀⠢⣌⠳⠜⠡⡘⣎⡞
// ⢆⢣⢐⣃⢦⠱⠊⣨⣀⣾⣿⣿⣿⡿⡷⠄⠛⠻⠿⣿⣿⣿⣿⣿⣿⣿⣿⣯⢿⣿⠀⡸⣿⣿⣿⣿⠟⠁⣴⣵⣿⠟⠁⣀⣼⡿⠉⣿⡀⠙⠗⡚⡁⠁⠠⡀⠀⠀⠀⠡⠈⠤⠑⡜⠜

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
        // // 1 cobblestone into 2 cobblestone and a diamond, -100000 priority
        Recipe::new(
            vec![(COBBLESTONE_ID, 1)],
            vec![(COBBLESTONE_ID, 2), (DIAMOND_ID, 1)],
            -100000),
    ]
}

fn get_starting_items() -> ItemSet {
    ItemSet::new(vec![
    (COBBLESTONE_ID, 20),
    ])
}

fn get_target() -> ItemSet {
    ItemSet::new(vec![(GLASS_ID, 11)])
}