use std::{
    cmp,
    collections::{HashMap, HashSet},
};

use good_lp::{
    Constraint, Expression, ProblemVariables, Solution, SolverModel, Variable, constraint,
    default_solver, variable,
};

use crate::{
    crafting_domain::{CraftingSolution, ItemId, ItemSet, Recipe, MAX_RECIPE_VALUE},
    execution_planner::build_executable_plan_from_recipe_usage,
    recipe_analysis::{
        collect_non_producible_items, collect_relevant_item_ids, detect_recipe_cycles,
        prioritize_and_prune_relevant_recipes_and_items,
        select_top_priority_recipes_per_output_item,
    },
};

type ExecutablePlan = Vec<(Recipe, usize)>;
type DisabledRecipeIdSet = HashSet<usize>;

enum RecipeVariableMode<'a> {
    Unbounded,
    DisabledAtZero(&'a HashSet<usize>),
    Capped(i32),
}

fn build_recipe_variables(
    recipes: &[Recipe],
    mode: RecipeVariableMode,
) -> (ProblemVariables, HashMap<usize, Variable>) {
    // Creates one LP integer variable per recipe with mode-specific bounds.
    // Returns both the variable registry and a recipe-id to variable lookup map.
    let mut recipe_to_variable = HashMap::new();
    let mut problem_variables = ProblemVariables::new();

    for recipe in recipes {
        let var = match mode {
            RecipeVariableMode::Unbounded => {
                problem_variables.add(variable().integer().min(0).name(recipe.describe()))
            }
            RecipeVariableMode::DisabledAtZero(disabled_recipe_ids) => {
                if disabled_recipe_ids.contains(&recipe.unique_id) {
                    problem_variables
                        .add(variable().integer().min(0).max(0).name(recipe.describe()))
                } else {
                    problem_variables.add(variable().integer().min(0).name(recipe.describe()))
                }
            }
            RecipeVariableMode::Capped(max_value) => problem_variables.add(
                variable()
                    .integer()
                    .min(0)
                    .max(max_value)
                    .name(recipe.describe()),
            ),
        };
        recipe_to_variable.insert(recipe.unique_id, var);
    }

    (problem_variables, recipe_to_variable)
}

fn build_item_flow_expressions_and_constraints<F>(
    recipes: &[Recipe],
    relevant_item_ids: &HashSet<ItemId>,
    starting_items: &ItemSet,
    target: &ItemSet,
    recipe_to_variable: &HashMap<usize, Variable>,
    missing_variable_message: &str,
    should_constrain_item: F,
) -> (HashMap<ItemId, Expression>, Vec<Constraint>)
where
    F: Fn(ItemId) -> bool,
{
    // Builds per-item inventory flow expressions and target constraints.
    // Item constraints are emitted only for item ids accepted by `should_constrain_item`.
    let mut item_expressions = HashMap::with_capacity(relevant_item_ids.len());
    let mut item_constraints = Vec::new();

    for item in relevant_item_ids {
        let mut constraint_expr = Expression::from(starting_items[*item] as i32);
        for recipe in recipes {
            let output_count = recipe.output[*item] as i32;
            let input_count = recipe.input[*item] as i32;
            let var = recipe_to_variable
                .get(&recipe.unique_id)
                .expect(missing_variable_message);
            constraint_expr = constraint_expr + output_count * *var - input_count * *var;
        }

        item_expressions.insert(*item, constraint_expr.clone());

        if should_constrain_item(*item) {
            item_constraints.push(constraint!(constraint_expr >= target[*item] as i32));
        }
    }

    (item_expressions, item_constraints)
}

fn lock_recipe_usages_in_priority_order(
    recipes: &[Recipe],
    problem_variables: &ProblemVariables,
    recipe_to_variable: &HashMap<usize, Variable>,
    item_constraints: &[Constraint],
    mut on_lock: impl FnMut(&Recipe, f64),
) -> Option<Vec<Constraint>> {
    // Solves lexicographically: minimizes each recipe usage in recipe order while locking prior values.
    // Returns the resulting equality constraints that pin recipe variables to chosen usage values.
    let mut recipe_constraints = Vec::new();

    problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.to_vec())
        .with_all(recipe_constraints.clone())
        .solve()
        .ok()?;

    for recipe in recipes {
        let var = recipe_to_variable.get(&recipe.unique_id)?;
        let solution = problem_variables
            .clone()
            .minimise(*var)
            .using(default_solver)
            .with_all(item_constraints.to_vec())
            .with_all(recipe_constraints.clone())
            .solve()
            .ok()?;

        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
        on_lock(recipe, var_value);
    }

    Some(recipe_constraints)
}

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

    let relevant_item_ids = collect_relevant_item_ids(&recipes, &target, &pruned_relevant_item_ids);

    let items_with_no_recipes = collect_non_producible_items(&recipes, &relevant_item_ids);

    println!(
        "Relaxed deficit analysis is using {} recipes and {} relevant items ({} with no recipes).",
        recipes.len(),
        relevant_item_ids.len(),
        items_with_no_recipes.len()
    );

    let (problem_variables, recipe_to_variable) =
        build_recipe_variables(&recipes, RecipeVariableMode::Unbounded);

    let (item_expressions, item_constraints) = build_item_flow_expressions_and_constraints(
        &recipes,
        &relevant_item_ids,
        &starting_items,
        &target,
        &recipe_to_variable,
        "Internal mapping error: recipe variable missing while building base-item constraints",
        |item_id| !items_with_no_recipes.contains(&item_id),
    );

    let recipe_constraints = lock_recipe_usages_in_priority_order(
        &recipes,
        &problem_variables,
        &recipe_to_variable,
        &item_constraints,
        |_, _| {},
    )
    .expect("Relaxed base-item model failed while minimizing recipe usage");

    let solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .expect("Relaxed base-item deficit model could not be solved");

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

    let (problem_variables, recipe_to_variable) =
        build_recipe_variables(&recipes, RecipeVariableMode::DisabledAtZero(disabled_recipe_ids));

    let (item_expressions, item_constraints) = build_item_flow_expressions_and_constraints(
        &recipes,
        &relevant_item_ids,
        &starting_items,
        &target,
        &recipe_to_variable,
        "Internal mapping error: recipe variable missing while building solve constraints",
        |_| true,
    );

    println!("Locking recipe usage one variable at a time...");
    let recipe_constraints = lock_recipe_usages_in_priority_order(
        &recipes,
        &problem_variables,
        &recipe_to_variable,
        &item_constraints,
        |recipe, var_value| println!("Locked usage {} for recipe '{}'", var_value, recipe.describe()),
    )?;

    println!("Re-solving model with locked usages to capture final inventories...");
    let solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve()
        .ok()?;

    println!("Usage locking complete for this branch.");

    let mut recipe_values = HashMap::new();
    for recipe in &recipes {
        if let Some(var) = recipe_to_variable.get(&recipe.unique_id) {
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

    let (problem_variables, recipe_to_variable) =
        build_recipe_variables(&recipes, RecipeVariableMode::Capped(MAX_RECIPE_VALUE));

    let (item_expressions, item_constraints) = build_item_flow_expressions_and_constraints(
        &recipes,
        &relevant_item_ids,
        &starting_items,
        &target,
        &recipe_to_variable,
        "Internal mapping error: recipe variable missing while building max-objective model",
        |_| true,
    );

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
