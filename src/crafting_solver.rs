use std::{
    cmp,
    collections::{HashMap, HashSet},
};

use good_lp::{
    Expression, ProblemVariables, Solution, SolverModel, constraint, default_solver, variable,
};

use crate::{
    crafting_domain::{CraftingSolution, ItemSet, Recipe, MAX_RECIPE_VALUE},
    execution_planner::build_executable_plan_from_recipe_usage,
    recipe_analysis::{
        collect_non_producible_items, detect_recipe_cycles,
        prioritize_and_prune_relevant_recipes_and_items,
        select_top_priority_recipes_per_output_item,
    },
};

type ExecutablePlan = Vec<(Recipe, usize)>;
type DisabledRecipeIdSet = HashSet<usize>;

pub fn compute_required_base_items(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> ItemSet {
    // Solves a relaxed feasibility model to estimate missing non-producible input items.
    // The solve then lexicographically minimizes recipe usage before calculating base-item deficits.
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
                "Internal mapping error: recipe variable missing while building base-item constraints",
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
        .expect("Relaxed base-item deficit model could not be solved");

    for recipe in &recipes {
        let var = recipe_to_variable.get(recipe).expect(
            "Internal mapping error: recipe variable missing during usage minimization",
        );
        solution = problem_variables
            .clone()
            .minimise(*var)
            .using(default_solver)
            .with_all(item_constraints.clone())
            .with_all(recipe_constraints.clone())
            .solve()
            .expect("Relaxed base-item model failed while minimizing recipe usage");

        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
    }

    let mut required = ItemSet::from_item_counts(vec![]);
    for item_id in &items_with_no_recipes {
        let final_inventory = item_expressions
            .get(item_id)
            .expect("Internal expression error: missing non-producible item expression")
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
    // Solves the main crafting model while forcing selected recipe ids to zero usage.
    // Returns `None` when constraints are infeasible under the current disabled set.
    println!("Pruning to target-relevant recipes and items...");
    let (recipes, relevant_item_ids) = prioritize_and_prune_relevant_recipes_and_items(recipes, &target);
    println!(
        "Constructing LP with {} recipe variables and {} item constraints...",
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
                "Internal mapping error: recipe variable missing while building solve constraints",
            );
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let mut recipe_constraints = Vec::new();
    println!("Solving baseline feasible model...");
    let mut solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .ok()?;

    println!("Baseline model solved. Locking recipe usage one variable at a time...");
    for recipe in &recipes {
        let var = recipe_to_variable
            .get(recipe)
            .expect("Internal mapping error: LP variable missing during usage locking");
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
        println!("Locked usage {} for recipe '{}'", var_value, recipe.describe());
    }
    println!("Usage locking complete for this branch.");

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
) -> Result<(CraftingSolution, ExecutablePlan), DisabledRecipeIdSet> {
    // Explores disabled-recipe branches until it finds a solve whose usages are executable in order.
    // When a branch is non-executable, it disables one high-usage recipe per detected used cycle.
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
            "Cycle-elimination branch #{} with disabled recipe IDs: {:?}",
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
                "- Branch #{} is infeasible with this disabled set; trying another branch.",
                attempt_index
            );
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }
        let solution = solution.expect("Branch feasibility was checked before unwrapping solution");

        if let Ok(plan) = build_executable_plan_from_recipe_usage(
            &recipes,
            &solution.recipe_values,
            &starting_items,
        ) {
            println!("- Branch #{} produced an executable crafting plan.", attempt_index);
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
                "- Branch #{} has no removable used cycles left; this branch is exhausted.",
                attempt_index
            );
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }

        println!(
            "- Branch #{} is non-executable; branching on {} detected used cycles.",
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
            "- Branch #{} spawned {} follow-up branch(es).",
            attempt_index, spawned_branches
        );
    }

    println!("Cycle-elimination search finished with no executable branch.");
    Err(best_fallback_disabled_recipe_ids)
}

pub fn compute_max_craftable_target_amount(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> usize {
    // Maximizes additional quantity of the first target item under inventory-flow constraints.
    // Returns the craftable amount above what already exists in starting inventory.
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
                "Internal mapping error: recipe variable missing while building max-objective model",
            );
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }
        item_expressions.insert(*item, constraint_expr.clone());
        item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
    }

    let objective = item_expressions
        .get(&target_item_id)
        .expect("Internal expression error: target item expression missing in max-objective model");

    println!("Solving max-craft objective for primary target item...");
    let solution = problem_variables
        .clone()
        .maximise(objective.clone())
        .using(default_solver)
        .with_all(item_constraints.clone())
        .solve();
    println!("Max-craft solve completed.");

    let Ok(solution) = solution else {
        return 0;
    };

    let result = objective.eval_with(&solution) - starting_items[target_item_id] as f64;
    if result.fract() != 0.0 {
        panic!("Solver returned non-integer primary target output: {}", result);
    }
    result as usize
}
