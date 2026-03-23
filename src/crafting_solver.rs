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

    crate::debugln!(
        "[debug] build_recipe_variables: created {} variables.",
        recipe_to_variable.len()
    );

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

    crate::debugln!(
        "[debug] build_item_flow_expressions_and_constraints: items={}, constraints={}",
        item_expressions.len(),
        item_constraints.len()
    );

    (item_expressions, item_constraints)
}

fn lock_recipe_usages_in_priority_order(
    recipes: &[Recipe],
    problem_variables: &ProblemVariables,
    recipe_to_variable: &HashMap<usize, Variable>,
    item_constraints: &[Constraint],
) -> Option<Vec<Constraint>> {
    // Solves lexicographically: minimizes each recipe usage in recipe order while locking prior values.
    // Returns the resulting equality constraints that pin recipe variables to chosen usage values.
    let mut recipe_constraints = Vec::new();

    crate::debugln!(
        "[debug] lock_recipe_usages_in_priority_order: initial feasibility solve starting (constraints={}).",
        item_constraints.len()
    );
    let initial_feasibility = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.to_vec())
        .with_all(recipe_constraints.clone())
        .solve();
    crate::debugln!(
        "[debug] lock_recipe_usages_in_priority_order: initial feasibility solve finished (ok={}).",
        initial_feasibility.is_ok()
    );
    initial_feasibility.ok()?;

    for recipe in recipes {
        let var = recipe_to_variable.get(&recipe.unique_id)?;
        crate::debugln!(
            "[debug] lock_recipe_usages_in_priority_order: solve starting for recipe '{}'.",
            recipe.describe()
        );
        let solution_result = problem_variables
            .clone()
            .minimise(*var)
            .using(default_solver)
            .with_all(item_constraints.to_vec())
            .with_all(recipe_constraints.clone())
            .solve();
        crate::debugln!(
            "[debug] lock_recipe_usages_in_priority_order: solve finished for recipe '{}' (ok={}).",
            recipe.describe(),
            solution_result.is_ok()
        );
        let solution = solution_result.ok()?;

        let var_value = solution.value(*var);
        recipe_constraints.push(constraint!(*var == var_value));
    }

    crate::debugln!(
        "[debug] lock_recipe_usages_in_priority_order: locked {} recipe variables.",
        recipe_constraints.len()
    );

    Some(recipe_constraints)
}

fn collect_loop_closing_recipe_ids_on_target_branches(
    recipes: &[Recipe],
    target: &ItemSet,
) -> HashSet<usize> {
    // Walks recipe-input branches backward from target items.
    // When a branch revisits an ancestor item, the current recipe is treated as loop-closing.
    // Returning those recipe ids allows callers to "pretend that recipe doesn't exist" and
    // reveal synthetic base-item deficits for fully-producible cyclic graphs.
    fn walk_item_dependencies(
        item_id: ItemId,
        output_to_recipes: &HashMap<ItemId, Vec<&Recipe>>,
        path_items: &mut HashSet<ItemId>,
        loop_closing_recipe_ids: &mut HashSet<usize>,
    ) {
        let Some(producing_recipes) = output_to_recipes.get(&item_id) else {
            return;
        };

        for recipe in producing_recipes {
            for input_item_id in recipe.input.items.keys() {
                if path_items.contains(input_item_id) {
                    loop_closing_recipe_ids.insert(recipe.unique_id);
                    continue;
                }

                path_items.insert(*input_item_id);
                walk_item_dependencies(
                    *input_item_id,
                    output_to_recipes,
                    path_items,
                    loop_closing_recipe_ids,
                );
                path_items.remove(input_item_id);
            }
        }
    }

    let mut output_to_recipes: HashMap<ItemId, Vec<&Recipe>> = HashMap::new();
    for recipe in recipes {
        for output_item_id in recipe.output.items.keys() {
            output_to_recipes
                .entry(*output_item_id)
                .or_default()
                .push(recipe);
        }
    }

    let mut loop_closing_recipe_ids = HashSet::<usize>::new();
    for target_item_id in target.items.keys() {
        let mut path_items = HashSet::<ItemId>::new();
        path_items.insert(*target_item_id);
        walk_item_dependencies(
            *target_item_id,
            &output_to_recipes,
            &mut path_items,
            &mut loop_closing_recipe_ids,
        );
    }

    crate::debugln!(
        "[debug] collect_loop_closing_recipe_ids_on_target_branches: found {} loop-closing recipes.",
        loop_closing_recipe_ids.len()
    );

    loop_closing_recipe_ids
}

pub fn compute_required_base_items(
    recipes: Vec<Recipe>,
    starting_items: ItemSet,
    target: ItemSet,
) -> ItemSet {
    // Tries to solve the problem except without any limits on non-producible items, meaning they can end up negative
    // Anything that ends up negative is something you need to add in order to get from starting to target, so we can report that as a base-item deficit.
    let (recipes, pruned_relevant_item_ids) =
        prioritize_and_prune_relevant_recipes_and_items(recipes, &target);
    let recipes = select_top_priority_recipes_per_output_item(&recipes);

    let relevant_item_ids = collect_relevant_item_ids(&recipes, &target, &pruned_relevant_item_ids);

    let items_with_no_recipes = collect_non_producible_items(&recipes, &relevant_item_ids);

    crate::debugln!(
        "[debug] compute_required_base_items: recipes={}, relevant-items={}, non-producible={}",
        recipes.len(),
        relevant_item_ids.len(),
        items_with_no_recipes.len()
    );

    if items_with_no_recipes.is_empty() {
        let loop_closing_recipe_ids =
            collect_loop_closing_recipe_ids_on_target_branches(&recipes, &target);
        if !loop_closing_recipe_ids.is_empty() {
            let reduced_recipes = recipes
                .iter()
                .filter(|recipe| !loop_closing_recipe_ids.contains(&recipe.unique_id))
                .cloned()
                .collect::<Vec<_>>();

            return compute_required_base_items(reduced_recipes, starting_items, target);
        }
    }

    println!(
        "Relaxed deficit analysis is using {} recipes and {} relevant items ({} with no recipes).",
        recipes.len(),
        relevant_item_ids.len(),
        items_with_no_recipes.len()
    );

    if recipes.is_empty() {
        let mut required = ItemSet::from_item_counts(vec![]);
        for item_id in &items_with_no_recipes {
            let needed = target[*item_id].saturating_sub(starting_items[*item_id]);
            if needed > 0 {
                required.add_count(*item_id, needed);
            }
        }
        return required;
    }

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
    )
    .expect("Relaxed base-item model failed while minimizing recipe usage");

    let solution = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone());
    crate::debugln!(
        "[debug] compute_required_base_items: final relaxed deficit solve starting."
    );
    let solution_result = solution.solve();
    crate::debugln!(
        "[debug] compute_required_base_items: final relaxed deficit solve finished (ok={}).",
        solution_result.is_ok()
    );
    let solution = solution_result.expect("Relaxed base-item deficit model could not be solved");

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

    crate::debugln!(
        "[debug] compute_required_base_items: required base items count={}",
        required.items.len()
    );

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
    let (recipes, relevant_item_ids) = prioritize_and_prune_relevant_recipes_and_items(recipes, &target);

    crate::debugln!(
        "[debug] solve_with_disabled_recipes: disabled={}, recipes-after-prune={}",
        disabled_recipe_ids.len(),
        recipes.len()
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

    let recipe_constraints = lock_recipe_usages_in_priority_order(
        &recipes,
        &problem_variables,
        &recipe_to_variable,
        &item_constraints,
    )?;

    crate::debugln!(
        "[debug] solve_with_disabled_recipes: final branch solve starting (constraints={}).",
        item_constraints.len() + recipe_constraints.len()
    );
    let solution_result = problem_variables
        .clone()
        .minimise(0)
        .using(default_solver)
        .with_all(item_constraints.clone())
        .with_all(recipe_constraints.clone())
        .solve();
    crate::debugln!(
        "[debug] solve_with_disabled_recipes: final branch solve finished (ok={}).",
        solution_result.is_ok()
    );
    let solution = solution_result.ok()?;

    crate::debugln!("[debug] solve_with_disabled_recipes: LP solve succeeded.");

    let mut recipe_values = HashMap::new();
    for recipe in &recipes {
        if let Some(var) = recipe_to_variable.get(&recipe.unique_id) {
            recipe_values.insert(recipe.unique_id, solution.value(*var));
        }
    }

    let mut final_inventory_values = ItemSet::from_item_counts(vec![]);
    for item_id in &relevant_item_ids {
        if let Some(expr) = item_expressions.get(item_id) {
            let value = expr.eval_with(&solution).round().max(0.0) as usize;
            final_inventory_values.add_count(*item_id, value);
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

    crate::debugln!(
        "[debug] find_executable_solution_via_cycle_elimination: starting with {} recipes.",
        recipes.len()
    );

    while let Some(disabled_recipe_ids) = attempts.pop() {
        attempt_index += 1;
        crate::debugln!(
            "[debug] cycle-elimination attempt #{} (disabled={}).",
            attempt_index,
            disabled_recipe_ids.len()
        );

        let solution = solve_with_disabled_recipes(
            recipes.clone(),
            starting_items.clone(),
            target.clone(),
            &disabled_recipe_ids,
        );
        if solution.is_none() {
            crate::debugln!("[debug] attempt #{} infeasible.", attempt_index);
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }
        let solution = solution.expect("Branch feasibility was checked before unwrapping solution");

        crate::debugln!(
            "[debug] attempt #{}: execution planning starting.",
            attempt_index
        );
        let planning_started_at = std::time::Instant::now();
        if let Ok(plan) = build_executable_plan_from_recipe_usage(
            &recipes,
            &solution.recipe_values,
            &starting_items,
        ) {
            crate::debugln!(
                "[debug] attempt #{}: execution planning finished (ok=true, steps={}, elapsed={:.3?}).",
                attempt_index,
                plan.len(),
                planning_started_at.elapsed()
            );
            return Ok((solution, plan));
        }
        crate::debugln!(
            "[debug] attempt #{}: execution planning finished (ok=false, elapsed={:.3?}).",
            attempt_index,
            planning_started_at.elapsed()
        );

        let used_recipes = recipes
            .iter()
            .filter_map(|recipe| {
                let value = solution
                    .recipe_values
                    .get(&recipe.unique_id)
                    .copied()
                    .unwrap_or(0.0);
                if value > 0.5 {
                    Some(recipe.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        crate::debugln!(
            "[debug] attempt #{}: cycle detection on used recipes starting (used-recipes={}).",
            attempt_index,
            used_recipes.len()
        );
        let used_cycle_detection_started_at = std::time::Instant::now();
        let (_, loops) = detect_recipe_cycles(&used_recipes);
        crate::debugln!(
            "[debug] attempt #{}: cycle detection on used recipes finished (loops={}, elapsed={:.3?}).",
            attempt_index,
            loops.len(),
            used_cycle_detection_started_at.elapsed()
        );
        crate::debugln!(
            "[debug] attempt #{}: used-recipes={}, detected-used-loops={}",
            attempt_index,
            used_recipes.len(),
            loops.len()
        );
        if loops.is_empty() {
            if disabled_recipe_ids.len() > best_fallback_disabled_recipe_ids.len() {
                best_fallback_disabled_recipe_ids = disabled_recipe_ids.clone();
            }
            continue;
        }

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
                }
            }
        }

    }
    crate::debugln!(
        "[debug] cycle-elimination exhausted branches. best-disabled-count={}",
        best_fallback_disabled_recipe_ids.len()
    );
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

    crate::debugln!(
        "[debug] compute_max_craftable_target_amount: solving with recipes={}, relevant-items={}",
        recipes.len(),
        relevant_item_ids.len()
    );
    crate::debugln!(
        "[debug] compute_max_craftable_target_amount: max-objective solve starting."
    );
    let solution = problem_variables
        .clone()
        .maximise(objective.clone())
        .using(default_solver)
        .with_all(item_constraints.clone())
        .solve();
    crate::debugln!(
        "[debug] compute_max_craftable_target_amount: max-objective solve finished (ok={}).",
        solution.is_ok()
    );

    let Ok(solution) = solution else {
        crate::debugln!("[debug] compute_max_craftable_target_amount: solve infeasible.");
        return 0;
    };

    let result = objective.eval_with(&solution) - starting_items[target_item_id] as f64;
    if result.fract() != 0.0 {
        panic!("Solver returned non-integer primary target output: {}", result);
    }
    result as usize
}
