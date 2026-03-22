use std::collections::HashMap;

use crate::{
    analysis::detect_recipe_cycles,
    domain::{ItemSet, Recipe},
};

pub fn build_executable_plan_from_recipe_usage(
    recipes: &[Recipe],
    recipe_values: &HashMap<Recipe, f64>,
    starting_items: &ItemSet,
) -> Result<Vec<(Recipe, usize)>, String> {
    // Converts aggregate recipe usage counts into a valid executable sequence of batched recipe applications.
    // Uses recursive backtracking to satisfy per-step input availability while honoring usage totals.
    fn compute_max_affordable_batch(recipe: &Recipe, inventory: &ItemSet) -> usize {
        // Computes the largest batch of `recipe` that can run with the current inventory.
        // The limiting reagent among all required inputs determines the max batch.
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
        // Applies one batched recipe execution by consuming inputs and adding outputs.
        // Mutates inventory in-place to reflect the new simulated state.
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
        // Reverts a previously applied batch during backtracking.
        // This restores inventory to the exact pre-step state for alternate branch exploration.
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
        // Appends a step to the plan, merging with the previous step when it is the same recipe.
        // This keeps the resulting plan concise and avoids adjacent duplicates.
        if let Some((last_recipe, last_batch)) = plan.last_mut() {
            if last_recipe.unique_id == recipe.unique_id {
                *last_batch += batch;
                return;
            }
        }
        plan.push((recipe.clone(), batch));
    }

    fn remove_or_shrink_last_plan_step(plan: &mut Vec<(Recipe, usize)>, recipe: &Recipe, batch: usize) {
        // Undoes the latest plan append/merge operation.
        // Used to keep plan state consistent with inventory when backtracking.
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

    fn recursively_backsolve_plan(
        recipes: &[Recipe],
        in_loop_by_id: &HashMap<usize, bool>,
        remaining_counts: &mut HashMap<usize, usize>,
        inventory: &mut ItemSet,
        total_remaining: usize,
        plan: &mut Vec<(Recipe, usize)>,
    ) -> bool {
        // Tries to complete all remaining recipe usages by DFS/backtracking over feasible batches.
        // Candidate ordering prioritizes loop-involved and high-impact batches to reduce dead ends.
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

    println!("Converting recipe usage counts into an executable crafting plan...");
    let mut remaining_counts: HashMap<usize, usize> = HashMap::new();
    for recipe in recipes {
        let raw_value = recipe_values.get(recipe).copied().unwrap_or(0.0);
        if raw_value < 0.0 {
            return Err(format!("Negative usage count for recipe '{}'", recipe.describe()));
        }

        let rounded = raw_value.round();
        if (raw_value - rounded).abs() > 1e-6 {
            return Err(format!(
                "Non-integer usage count {} for recipe '{}'",
                raw_value,
                recipe.describe()
            ));
        }

        let count = rounded as usize;
        if count > 0 {
            remaining_counts.insert(recipe.unique_id, count);
        }
    }

    let (in_loop_by_recipe, _) = detect_recipe_cycles(recipes);
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

    if recursively_backsolve_plan(
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
                Some(format!("{} (remaining {})", recipe.describe(), remaining))
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
