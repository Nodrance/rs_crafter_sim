use std::collections::HashMap;

use crate::{
    crafting_domain::{ItemSet, Recipe},
    progress_logger::PeriodicLogger,
    recipe_analysis::detect_recipe_cycles,
};

pub fn build_executable_plan_from_recipe_usage(
    recipes: &[Recipe],
    recipe_values: &HashMap<usize, f64>,
    starting_items: &ItemSet,
) -> Result<Vec<(Recipe, usize)>, String> {
    // Translates solved recipe usage totals into an execution-safe sequence of recipe batches.
    // Uses backtracking against live inventory so every step remains craftable when applied in order.
    fn compute_max_affordable_batch(recipe: &Recipe, inventory: &ItemSet) -> usize {
        // Returns the largest batch count currently affordable for this recipe.
        // The smallest input ratio acts as the limiting resource.
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

    fn apply_recipe_batch(recipe: &Recipe, batch: usize, inventory: &mut ItemSet) {
        // Applies one candidate step by consuming inputs and adding outputs.
        // This mutates the simulation inventory used by the search.
        for (item_id, input_count) in &recipe.input.items {
            let required = input_count * batch;
            let available_entry = inventory.items.entry(*item_id).or_insert(0);
            *available_entry -= required;
        }

        for (item_id, output_count) in &recipe.output.items {
            inventory.add_count(*item_id, output_count * batch);
        }
    }

    fn rollback_recipe_batch(recipe: &Recipe, batch: usize, inventory: &mut ItemSet) {
        // Reverts the last applied candidate step during backtracking.
        // This restores inventory exactly before trying alternate branches.
        for (item_id, output_count) in &recipe.output.items {
            let produced = output_count * batch;
            let available_entry = inventory.items.entry(*item_id).or_insert(0);
            *available_entry -= produced;
        }

        for (item_id, input_count) in &recipe.input.items {
            inventory.add_count(*item_id, input_count * batch);
        }
    }

    fn append_or_merge_plan_step(plan: &mut Vec<(Recipe, usize)>, recipe: &Recipe, batch: usize) {
        // Appends a step, or merges it into the previous step when recipe ids match.
        // This keeps the final execution plan compact without changing semantics.
        if let Some((last_recipe, last_batch)) = plan.last_mut()
            && last_recipe.unique_id == recipe.unique_id
        {
            *last_batch += batch;
            return;
        }
        plan.push((recipe.clone(), batch));
    }

    fn remove_or_shrink_last_plan_step(plan: &mut Vec<(Recipe, usize)>, recipe: &Recipe, batch: usize) {
        // Reverses the latest step insertion/merge to keep plan state in sync with rollback.
        if let Some((last_recipe, last_batch)) = plan.last_mut()
            && last_recipe.unique_id == recipe.unique_id
        {
            if *last_batch == batch {
                plan.pop();
            } else {
                *last_batch -= batch;
            }
        }
    }

    struct BacksolveProgress {
        logger: PeriodicLogger,
        explored_states: usize,
    }

    fn recursively_backsolve_plan(
        recipes: &[Recipe],
        in_loop_by_id: &HashMap<usize, bool>,
        remaining_counts: &mut HashMap<usize, usize>,
        inventory: &mut ItemSet,
        total_remaining: usize,
        plan: &mut Vec<(Recipe, usize)>,
        progress: &mut BacksolveProgress,
    ) -> bool {
        // Recursively satisfies remaining usage counts while preserving craftability at each step.
        // Candidate ordering favors loop-related and larger moves to reduce search churn.
        progress.explored_states += 1;
        progress.logger.heartbeat(&format!(
            "[debug] backsolve progress: explored-states={}, remaining-applications={}, plan-steps={}",
            progress.explored_states, total_remaining, plan.len()
        ));

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

                let max_batch = compute_max_affordable_batch(recipe, inventory);
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
                .then_with(|| {
                    recipe_a
                        .effective_priority
                        .unwrap_or(isize::MAX)
                        .cmp(&recipe_b.effective_priority.unwrap_or(isize::MAX))
                })
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

                apply_recipe_batch(recipe, batch, inventory);
                remaining_counts.insert(recipe.unique_id, remaining - batch);
                append_or_merge_plan_step(plan, recipe, batch);

                if recursively_backsolve_plan(
                    recipes,
                    in_loop_by_id,
                    remaining_counts,
                    inventory,
                    total_remaining - batch,
                    plan,
                    progress,
                ) {
                    return true;
                }

                remove_or_shrink_last_plan_step(plan, recipe, batch);
                remaining_counts.insert(recipe.unique_id, remaining);
                rollback_recipe_batch(recipe, batch, inventory);
            }
        }

        false
    }

    let mut remaining_counts: HashMap<usize, usize> = HashMap::new();
    for recipe in recipes {
        let raw_value = recipe_values.get(&recipe.unique_id).copied().unwrap_or(0.0);
        if raw_value < 0.0 {
            return Err(format!(
                "Solver produced a negative usage count for recipe '{}'",
                recipe.describe()
            ));
        }

        let rounded = raw_value.round();
        if (raw_value - rounded).abs() > 1e-6 {
            return Err(format!(
                "Solver produced non-integer usage count {} for recipe '{}'",
                raw_value,
                recipe.describe()
            ));
        }

        let count = rounded as usize;
        if count > 0 {
            remaining_counts.insert(recipe.unique_id, count);
        }
    }

    crate::debugln!(
        "[debug] build_executable_plan_from_recipe_usage: cycle detection for planning starting (recipes={}).",
        recipes.len()
    );
    let cycle_detection_started_at = std::time::Instant::now();
    let (in_loop_by_recipe, loops) = detect_recipe_cycles(recipes);
    crate::debugln!(
        "[debug] build_executable_plan_from_recipe_usage: cycle detection finished (loops={}, elapsed={:.3?}).",
        loops.len(),
        cycle_detection_started_at.elapsed()
    );
    let in_loop_by_id = recipes
        .iter()
        .map(|recipe| {
            (
                recipe.unique_id,
                in_loop_by_recipe.get(&recipe.unique_id).copied().unwrap_or(false),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut inventory = starting_items.clone();
    let total_remaining: usize = remaining_counts.values().sum();
    let mut plan: Vec<(Recipe, usize)> = Vec::new();
    let mut progress = BacksolveProgress {
        logger: PeriodicLogger::new(std::time::Duration::from_millis(750)),
        explored_states: 0,
    };
    let planning_started_at = std::time::Instant::now();

    crate::debugln!(
        "[debug] build_executable_plan_from_recipe_usage: recipes={}, required-applications={}",
        recipes.len(),
        total_remaining
    );
    crate::debugln!(
        "[debug] build_executable_plan_from_recipe_usage: backsolve planning starting."
    );

    if recursively_backsolve_plan(
        recipes,
        &in_loop_by_id,
        &mut remaining_counts,
        &mut inventory,
        total_remaining,
        &mut plan,
        &mut progress,
    ) {
        crate::debugln!(
            "[debug] build_executable_plan_from_recipe_usage: backsolve planning finished (found=true, explored-states={}, elapsed={:.3?}).",
            progress.explored_states,
            planning_started_at.elapsed()
        );
        crate::debugln!(
            "[debug] build_executable_plan_from_recipe_usage: plan found with {} steps.",
            plan.len()
        );
        return Ok(plan);
    }

    crate::debugln!(
        "[debug] build_executable_plan_from_recipe_usage: backsolve planning finished (found=false, explored-states={}, elapsed={:.3?}).",
        progress.explored_states,
        planning_started_at.elapsed()
    );

    let blocked = recipes
        .iter()
        .filter_map(|recipe| {
            let remaining = remaining_counts.get(&recipe.unique_id).copied().unwrap_or(0);
            if remaining > 0 {
                Some(format!("{} (remaining {})", recipe.describe(), remaining))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "No valid execution order could satisfy all solved usages. Remaining blocked recipes: {}",
        blocked
    ))
}
