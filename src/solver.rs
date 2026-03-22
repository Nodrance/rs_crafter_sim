use std::{
    cmp,
    collections::{HashMap, HashSet},
};

use good_lp::{
    Expression, ProblemVariables, Solution, SolverModel, constraint, default_solver, variable,
};

use crate::{
    analysis::{
        collect_non_producible_items, detect_recipe_cycles,
        prioritize_and_prune_relevant_recipes_and_items,
        select_top_priority_recipes_per_output_item,
    },
    domain::{CraftingSolution, ItemSet, Recipe, MAX_RECIPE_VALUE},
    planner::build_executable_plan_from_recipe_usage,
};

pub fn compute_required_base_items(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> ItemSet {
    // Solves a relaxed model to identify which non-producible items are additionally required.
    // It minimizes recipe usage in priority order, then computes remaining deficits for base items only.
    let (recipes, pruned_relevant_item_ids) =
        prioritize_and_prune_relevant_recipes_and_items(recipes, &target);
    let recipes = select_top_priority_recipes_per_output_item(&recipes);

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

    let items_with_no_recipes = collect_non_producible_items(&recipes, &relevant_item_ids);

    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = problem_variables.add(variable().integer().min(0).name(recipe.describe()));
        recipe_to_variable.insert(recipe.clone(), var);
    }

    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();
    for item in &relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in &recipes {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable.get(recipe).expect(
                "A recipe is being used in compute_required_base_items that has no variable attached",
            );
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
        let var = recipe_to_variable.get(recipe).expect(
            "A recipe is being minimized in compute_required_base_items that has no variable attached",
        );
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

    let mut required = ItemSet::from_item_counts(vec![]);
    for item_id in &items_with_no_recipes {
        let final_inventory = item_expressions
            .get(item_id)
            .expect("Missing expression for non-producible item in compute_required_base_items")
            .eval_with(&solution);
        let needed = (target[*item_id] as f64 - final_inventory).ceil().max(0.0) as usize;
        if needed > 0 {
            required.add_count(*item_id, needed);
        }
    }

    required
}

fn solve_with_disabled_recipes(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
    disabled_recipe_ids: &HashSet<usize>,
) -> Option<CraftingSolution> {
    // Builds and solves the constrained LP while forcing selected recipe ids to zero usage.
    // Returns solved recipe usage and final inventory values if the model is feasible.
    println!("Pruning irrelevant recipes and items...");
    let (recipes, relevant_item_ids) = prioritize_and_prune_relevant_recipes_and_items(recipes, &target);
    println!(
        "Setting up linear program with {} variables and {} constraints...",
        recipes.len(),
        relevant_item_ids.len()
    );

    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = if disabled_recipe_ids.contains(&recipe.unique_id) {
            problem_variables.add(variable().integer().min(0).max(0).name(recipe.describe()))
        } else {
            problem_variables.add(variable().integer().min(0).name(recipe.describe()))
        };
        recipe_to_variable.insert(recipe.clone(), var);
    }

    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();
    for item in &relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in &recipes {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable.get(recipe).expect(
                "A recipe is being used in solve_with_disabled_recipes that has no variable attached",
            );
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let mut recipe_constraints = Vec::new();
    println!("Setting up initial solution...");
    let mut solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .ok()?;

    println!("Initial solution found. Locking in recipe usage one by one based on priority...");
    for recipe in &recipes {
        let var = recipe_to_variable
            .get(recipe)
            .expect("Missing LP variable while minimizing recipe usage in solve_with_disabled_recipes");
        solution = problem_variables
            .clone()
            .minimise(*var)
            .using(default_solver)
            .with_all(item_constraints.clone())
            .with_all(recipe_constraints.clone())
            .solve()
            .ok()?;
        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
        println!("Locked in {} usages for recipe '{}'", var_value, recipe.describe());
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

pub fn find_executable_solution_via_cycle_elimination(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> Result<(CraftingSolution, Vec<(Recipe, usize)>), HashSet<usize>> {
    // Searches for an LP solution whose recipe usages can be executed in valid order.
    // If a solution is not executable, branches by disabling one recipe from each detected used cycle.
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
            attempt_index, disabled_sorted
        );

        let solution = solve_with_disabled_recipes(
            recipes.clone(),
            starting_items.clone(),
            target.clone(),
            &disabled_recipe_ids,
        );
        if solution.is_none() {
            println!(
                "- Attempt #{} infeasible after disabled set, trying next branch.",
                attempt_index
            );
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }
        let solution = solution.expect("Solution checked above as Some");

        if let Ok(plan) = build_executable_plan_from_recipe_usage(
            &recipes,
            &solution.recipe_values,
            &starting_items,
        ) {
            println!("- Attempt #{} produced an executable plan.", attempt_index);
            return Ok((solution, plan));
        }

        let used_recipes = solution
            .recipe_values
            .iter()
            .filter_map(|(recipe, value)| if *value > 0.5 { Some(recipe.clone()) } else { None })
            .collect::<Vec<_>>();

        let (_, loops) = detect_recipe_cycles(&used_recipes);
        if loops.is_empty() {
            println!(
                "- Attempt #{} had no used loops to eliminate; no more progress from this branch.",
                attempt_index
            );
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
                    let value_a = solution.recipe_usage_count(recipe_a);
                    let value_b = solution.recipe_usage_count(recipe_b);
                    value_a.partial_cmp(&value_b).unwrap_or(cmp::Ordering::Equal)
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
            attempt_index, spawned_branches
        );
    }

    println!("Cycle-elimination search exhausted all branches with no executable plan.");
    Err(best_fallback_disabled_recipe_ids)
}

pub fn compute_max_craftable_target_amount(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> usize {
    // Maximizes production of the first target item under inventory-flow constraints.
    // Returns how many additional units beyond the starting inventory are craftable.
    let (recipes, relevant_item_ids) = prioritize_and_prune_relevant_recipes_and_items(recipes, &target);
    let target_item_id = *target
        .items
        .keys()
        .next()
        .expect("Target item set is empty in compute_max_craftable_target_amount");

    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();
    for recipe in &recipes {
        let var = problem_variables.add(
            variable()
                .integer()
                .min(0)
                .max(MAX_RECIPE_VALUE)
                .name(recipe.describe()),
        );
        recipe_to_variable.insert(recipe.clone(), var);
    }

    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();
    for item in &relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in &recipes {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable.get(recipe).expect(
                "A recipe is being used in compute_max_craftable_target_amount that has no variable attached",
            );
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let objective = item_expressions
        .get(&target_item_id)
        .expect("A target item is being used in compute_max_craftable_target_amount that has no expression attached");

    println!("Solving for maximum number of target items craftable...");
    let solution = problem_variables
        .clone()
        .maximise(objective.clone())
        .using(default_solver)
        .with_all(item_constraints.clone())
        .solve();
    println!("Solution process complete.");

    let Ok(solution) = solution else {
        return 0;
    };

    let result = objective.eval_with(&solution) - starting_items[target_item_id] as f64;
    if result.fract() != 0.0 {
        panic!("Solution is somehow not an integer: {}", result);
    }
    result as usize
}
