const MAX_RECIPE_VALUE: i32 = i32::MAX - 10000; // A large number to use as the upper bound for recipe usage variables, to prevent overflow in the solver while still allowing for very large usage counts when maximizing


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

fn find_recipe_loops(recipes: &[Recipe]) -> (HashMap<Recipe, bool>, Vec<Vec<Recipe>>) {
    fn shares_output_to_input(from: &Recipe, to: &Recipe) -> bool {
        from.output
            .items
            .keys()
            .any(|item_id| to.input[*item_id] > 0)
    }

    fn canonical_cycle(cycle: &[usize]) -> Vec<usize> {
        if cycle.is_empty() {
            return Vec::new();
        }

        let mut best = cycle.to_vec();
        for shift in 1..cycle.len() {
            let mut rotated = Vec::with_capacity(cycle.len());
            rotated.extend_from_slice(&cycle[shift..]);
            rotated.extend_from_slice(&cycle[..shift]);
            if rotated < best {
                best = rotated;
            }
        }
        best
    }

    fn dfs_find_cycles(
        start: usize,
        current: usize,
        adjacency: &[Vec<usize>],
        on_path: &mut [bool],
        path: &mut Vec<usize>,
        seen_cycles: &mut HashSet<Vec<usize>>,
        cycles: &mut Vec<Vec<usize>>,
    ) {
        for &next in &adjacency[current] {
            if next == start && path.len() > 1 {
                let cycle = canonical_cycle(path);
                if seen_cycles.insert(cycle.clone()) {
                    cycles.push(cycle);
                }
                continue;
            }

            if on_path[next] {
                continue;
            }

            if path.len() >= adjacency.len() {
                continue;
            }

            on_path[next] = true;
            path.push(next);
            dfs_find_cycles(start, next, adjacency, on_path, path, seen_cycles, cycles);
            path.pop();
            on_path[next] = false;
        }
    }

    let mut adjacency = vec![Vec::new(); recipes.len()];
    for (from_index, from_recipe) in recipes.iter().enumerate() {
        for (to_index, to_recipe) in recipes.iter().enumerate() {
            if shares_output_to_input(from_recipe, to_recipe) {
                adjacency[from_index].push(to_index);
            }
        }
    }

    let mut seen_cycles: HashSet<Vec<usize>> = HashSet::new();
    let mut cycle_indices: Vec<Vec<usize>> = Vec::new();

    for start in 0..recipes.len() {
        let mut on_path = vec![false; recipes.len()];
        let mut path = vec![start];
        on_path[start] = true;
        dfs_find_cycles(
            start,
            start,
            &adjacency,
            &mut on_path,
            &mut path,
            &mut seen_cycles,
            &mut cycle_indices,
        );
    }

    cycle_indices.sort();

    let mut in_loop_by_recipe: HashMap<Recipe, bool> = HashMap::new();
    for recipe in recipes {
        in_loop_by_recipe.insert(recipe.clone(), false);
    }

    let mut loops = Vec::with_capacity(cycle_indices.len());
    for cycle in cycle_indices {
        let mut loop_recipes = Vec::with_capacity(cycle.len());
        for recipe_index in cycle {
            let recipe = recipes[recipe_index].clone();
            in_loop_by_recipe.insert(recipe.clone(), true);
            loop_recipes.push(recipe);
        }
        loops.push(loop_recipes);
    }

    (in_loop_by_recipe, loops)
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

fn calculate_solution_with_disabled_recipes(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
    disabled_recipe_ids: &HashSet<usize>,
) -> Option<CraftingSolution> {
    println!("Pruning irrelevant recipes and items...");
    let (recipes, relevant_item_ids) = sort_and_prune_relevant_recipes_and_items(recipes, &target);
    println!("Setting up linear program with {} variables and {} constraints...", recipes.len(), relevant_item_ids.len());
    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = if disabled_recipe_ids.contains(&recipe.unique_id) {
            problem_variables.add(variable().integer().min(0).max(0).name(recipe.name()))
        } else {
            problem_variables.add(variable().integer().min(0).name(recipe.name()))
        };
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
        .solve()
        .ok()?;
    println!("Initial solution found. Locking in recipe usage one by one based on priority...");
    for recipe in &recipes {
        let var = recipe_to_variable.get(recipe).unwrap();
        solution = problem_variables.clone()
        .minimise(*var).using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .ok()?;
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

    Some(CraftingSolution {
        recipe_values,
        final_inventory_values,
        relevant_item_ids,
    })
}

fn find_executable_solution_with_cycle_elimination(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> Result<(CraftingSolution, Vec<(Recipe, usize)>), HashSet<usize>> {
    let mut attempts = vec![HashSet::<usize>::new()];
    let mut visited: HashSet<Vec<usize>> = HashSet::new();
    visited.insert(Vec::new());
    let mut attempt_index: usize = 0;
    let mut best_fallback_disabled_recipe_ids = HashSet::<usize>::new();

    while let Some(disabled_recipe_ids) = attempts.pop() {
        attempt_index += 1;
        let mut disabled_sorted = disabled_recipe_ids.iter().copied().collect::<Vec<_>>();
        disabled_sorted.sort_unstable();
        println!(
            "Cycle-elimination attempt #{} with disabled recipe IDs: {:?}",
            attempt_index,
            disabled_sorted
        );

        let solution = calculate_solution_with_disabled_recipes(
            recipes.clone(),
            starting_items.clone(),
            target.clone(),
            &disabled_recipe_ids,
        );
        if solution.is_none() {
            println!("- Attempt #{} infeasible after disabled set, trying next branch.", attempt_index);
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }
        let solution = solution.unwrap();

        if let Ok(plan) = recipe_usage_to_application_plan(&recipes, &solution.recipe_values, &starting_items) {
            println!("- Attempt #{} produced an executable plan.", attempt_index);
            return Ok((solution, plan));
        }

        let used_recipes = solution
            .recipe_values
            .iter()
            .filter_map(|(recipe, value)| if *value > 0.5 { Some(recipe.clone()) } else { None })
            .collect::<Vec<_>>();
        let (_, loops) = find_recipe_loops(&used_recipes);
        if loops.is_empty() {
            println!("- Attempt #{} had no used loops to eliminate; no more progress from this branch.", attempt_index);
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }

        println!(
            "- Attempt #{} not executable; found {} used loop routes to branch on.",
            attempt_index,
            loops.len()
        );

        let mut spawned_branches = 0usize;

        for loop_route in loops {
            let recipe_to_disable = loop_route
                .iter()
                .max_by(|recipe_a, recipe_b| {
                    let value_a = solution.recipe_value(recipe_a);
                    let value_b = solution.recipe_value(recipe_b);
                    value_a
                        .partial_cmp(&value_b)
                        .unwrap_or(cmp::Ordering::Equal)
                })
                .map(|recipe| recipe.unique_id);

            if let Some(recipe_id) = recipe_to_disable {
                if disabled_recipe_ids.contains(&recipe_id) {
                    continue;
                }

                let mut next_disabled = disabled_recipe_ids.clone();
                next_disabled.insert(recipe_id);
                let mut key = next_disabled.iter().copied().collect::<Vec<_>>();
                key.sort_unstable();
                if visited.insert(key) {
                    attempts.push(next_disabled);
                    spawned_branches += 1;
                }
            }
        }

        println!(
            "- Attempt #{} spawned {} new cycle-elimination branches.",
            attempt_index,
            spawned_branches
        );
    }

    println!("Cycle-elimination search exhausted all branches with no executable plan.");
    Err(best_fallback_disabled_recipe_ids)
}

fn recipe_usage_to_application_plan(
    recipes: &[Recipe],
    recipe_values: &HashMap<Recipe, f64>,
    starting_items: &ItemSet,
) -> Result<Vec<(Recipe, usize)>, String> {
    fn max_batch_for_recipe(recipe: &Recipe, inventory: &ItemSet) -> usize {
        let mut max_batch = usize::MAX;
        for (item_id, input_count) in &recipe.input.items {
            if *input_count == 0 {
                continue;
            }
            let available = inventory[*item_id];
            max_batch = max_batch.min(available / *input_count);
        }
        max_batch
    }

    fn apply_batch(recipe: &Recipe, batch: usize, inventory: &mut ItemSet) {
        for (item_id, input_count) in &recipe.input.items {
            let required = input_count * batch;
            let available_entry = inventory.items.entry(*item_id).or_insert(0);
            *available_entry -= required;
        }

        for (item_id, output_count) in &recipe.output.items {
            inventory.add(*item_id, output_count * batch);
        }
    }

    fn unapply_batch(recipe: &Recipe, batch: usize, inventory: &mut ItemSet) {
        for (item_id, output_count) in &recipe.output.items {
            let produced = output_count * batch;
            let available_entry = inventory.items.entry(*item_id).or_insert(0);
            *available_entry -= produced;
        }

        for (item_id, input_count) in &recipe.input.items {
            inventory.add(*item_id, input_count * batch);
        }
    }

    fn push_plan_step(plan: &mut Vec<(Recipe, usize)>, recipe: &Recipe, batch: usize) {
        if let Some((last_recipe, last_batch)) = plan.last_mut() {
            if last_recipe.unique_id == recipe.unique_id {
                *last_batch += batch;
                return;
            }
        }
        plan.push((recipe.clone(), batch));
    }

    fn pop_plan_step(plan: &mut Vec<(Recipe, usize)>, recipe: &Recipe, batch: usize) {
        if let Some((last_recipe, last_batch)) = plan.last_mut() {
            if last_recipe.unique_id == recipe.unique_id {
                if *last_batch == batch {
                    plan.pop();
                } else {
                    *last_batch -= batch;
                }
            }
        }
    }

    fn backsolve_plan(
        recipes: &[Recipe],
        in_loop_by_id: &HashMap<usize, bool>,
        remaining_counts: &mut HashMap<usize, usize>,
        inventory: &mut ItemSet,
        total_remaining: usize,
        plan: &mut Vec<(Recipe, usize)>,
    ) -> bool {
        if total_remaining == 0 {
            return true;
        }

        let mut candidates: Vec<(&Recipe, usize, usize)> = recipes
            .iter()
            .filter_map(|recipe| {
                let remaining = remaining_counts.get(&recipe.unique_id).copied().unwrap_or(0);
                if remaining == 0 {
                    return None;
                }

                let max_batch = max_batch_for_recipe(recipe, inventory);
                if max_batch == 0 {
                    return None;
                }

                Some((recipe, remaining, max_batch.min(remaining)))
            })
            .collect();

        if candidates.is_empty() {
            return false;
        }

        candidates.sort_by(|(recipe_a, remaining_a, batch_a), (recipe_b, remaining_b, batch_b)| {
            let in_loop_a = in_loop_by_id.get(&recipe_a.unique_id).copied().unwrap_or(false);
            let in_loop_b = in_loop_by_id.get(&recipe_b.unique_id).copied().unwrap_or(false);

            in_loop_b
                .cmp(&in_loop_a)
                .then_with(|| batch_b.cmp(batch_a))
                .then_with(|| remaining_b.cmp(remaining_a))
                .then_with(|| recipe_a.effective_priority.unwrap_or(isize::MAX).cmp(&recipe_b.effective_priority.unwrap_or(isize::MAX)))
                .then_with(|| recipe_a.unique_id.cmp(&recipe_b.unique_id))
        });

        for (recipe, remaining, max_batch) in candidates {
            let mut try_batches = vec![max_batch];
            if max_batch > 1 {
                try_batches.push(1);
            }
            if max_batch > 2 {
                try_batches.push(max_batch / 2);
            }
            try_batches.sort_unstable();
            try_batches.dedup();
            try_batches.reverse();

            for batch in try_batches {
                if batch == 0 || batch > remaining {
                    continue;
                }

                apply_batch(recipe, batch, inventory);
                remaining_counts.insert(recipe.unique_id, remaining - batch);
                push_plan_step(plan, recipe, batch);

                if backsolve_plan(
                    recipes,
                    in_loop_by_id,
                    remaining_counts,
                    inventory,
                    total_remaining - batch,
                    plan,
                ) {
                    return true;
                }

                pop_plan_step(plan, recipe, batch);
                remaining_counts.insert(recipe.unique_id, remaining);
                unapply_batch(recipe, batch, inventory);
            }
        }

        false
    }

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

    let (in_loop_by_recipe, _) = find_recipe_loops(recipes);
    let in_loop_by_id = recipes
        .iter()
        .map(|recipe| {
            (
                recipe.unique_id,
                in_loop_by_recipe.get(recipe).copied().unwrap_or(false),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut inventory = starting_items.clone();
    let total_remaining: usize = remaining_counts.values().sum();
    let mut plan: Vec<(Recipe, usize)> = Vec::new();

    if backsolve_plan(
        recipes,
        &in_loop_by_id,
        &mut remaining_counts,
        &mut inventory,
        total_remaining,
        &mut plan,
    ) {
        return Ok(plan);
    }

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

    Err(format!(
        "Could not produce a valid execution order with recursive backsolving. Blocked recipes: {}",
        blocked
    ))
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
        let var = problem_variables.add(variable().integer().min(0).max(MAX_RECIPE_VALUE).name(recipe.name()));
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
    let executable_or_fallback = find_executable_solution_with_cycle_elimination(
        recipes.clone(),
        starting_items.clone(),
        target.clone(),
    );

    if executable_or_fallback.is_err() {
        let disabled_recipe_ids = executable_or_fallback.err().unwrap_or_default();
        let fallback_recipes = recipes
            .iter()
            .filter(|recipe| !disabled_recipe_ids.contains(&recipe.unique_id))
            .cloned()
            .collect::<Vec<_>>();

        println!("No executable plan could be found after eliminating loop usage. Falling back to required-items analysis.");
        println!(
            "Required-items analysis will use {} recipes after cycle elimination.",
            fallback_recipes.len()
        );
        let required_items = get_required_items(fallback_recipes, starting_items, target);
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

    let (solution, plan) = executable_or_fallback.unwrap();
    println!("Successfully crafted the target item!");

    println!("\nRecipe usage breakdown:");
    for recipe in &recipes {
        let var_value = solution.recipe_value(recipe);
        if var_value == 0.0 {continue;}
        println!("- {}: {}", recipe.name(), var_value);
    }

    println!("\nExecutable recipe plan:");
    for (recipe, count) in plan {
        println!("- Apply '{}' x{}", recipe.name(), count);
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
    (COBBLESTONE_ID, 0),
    ])
}

fn get_target() -> ItemSet {
    ItemSet::new(vec![(GLASS_ID, 11)])
}