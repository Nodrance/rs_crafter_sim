use std::collections::{HashMap, HashSet};

use crate::crafting_domain::{ItemId, ItemSet, Recipe, RecipePriorityKey};

pub fn prioritize_and_prune_relevant_recipes_and_items(recipes: Vec<Recipe>, target: &ItemSet) -> (Vec<Recipe>, HashSet<ItemId>) {
    // Walks backward from target items to find only reachable recipes/items.
    // Assigns an effective priority order to each retained recipe based on the best path key.
    let mut best_item_priorities: HashMap<ItemId, RecipePriorityKey> = HashMap::new();
    let mut best_recipe_priorities: HashMap<Recipe, RecipePriorityKey> = HashMap::new();
    let mut stack = Vec::new();

    for item_id in target.items.keys() {
        best_item_priorities.insert(*item_id, RecipePriorityKey(Vec::new()));
        stack.push(*item_id);
    }

    while let Some(output_item_id) = stack.pop() {
        let output_priority = best_item_priorities
            .get(&output_item_id)
            .cloned()
            .expect("Error during recipe sort/prune: Queued items must already have a priority. This should be unreachable.");

        for recipe in recipes.iter() {
            if !recipe.output.items.iter().any(|(&item_id, _)| item_id == output_item_id) {
                continue;
            }

            let mut candidate_recipe_priority = output_priority.clone();
            candidate_recipe_priority.append_recipe_priority(recipe);

            let should_update_recipe = best_recipe_priorities
                .get(recipe)
                .map(|current| candidate_recipe_priority < *current)
                .unwrap_or(true);
            if !should_update_recipe {
                continue;
            }
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

    let mut pruned_recipes = best_recipe_priorities.into_iter().collect::<Vec<_>>();
    pruned_recipes.sort_by_key(|(_, priority)| priority.clone());
    for (index, (recipe, _)) in pruned_recipes.iter_mut().enumerate() {
        recipe.effective_priority = Some(index as isize);
    }
    let recipes = pruned_recipes
        .into_iter()
        .map(|(recipe, _)| recipe)
        .collect::<Vec<_>>();

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

pub fn collect_non_producible_items(recipes: &[Recipe], relevant_item_ids: &HashSet<ItemId>) -> HashSet<ItemId> {
    // Returns relevant items that no recipe can output.
    // These items represent external/base resources that must already exist or be added.
    relevant_item_ids
        .iter()
        .copied()
        .filter(|item_id| !recipes.iter().any(|recipe| recipe.output[*item_id] > 0))
        .collect()
}

pub fn select_top_priority_recipes_per_output_item(recipes: &[Recipe]) -> Vec<Recipe> {
    // Keeps a minimal set of recipes by selecting only those that introduce at least one unseen output item.
    // Input order is derived from effective priority so lower-priority options are naturally discarded.
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

pub fn detect_recipe_cycles(recipes: &[Recipe]) -> (HashMap<Recipe, bool>, Vec<Vec<Recipe>>) {
    // Detects cycles in the recipe dependency graph where one recipe's outputs feed another's inputs.
    // Returns both per-recipe cycle membership and canonicalized cycle routes.
    fn recipe_outputs_feed_recipe_inputs(from: &Recipe, to: &Recipe) -> bool {
        // Checks whether any output item from `from` is consumed as input by `to`.
        // This defines a directed graph edge from `from` to `to`.
        from.output.items.keys().any(|item_id| to.input[*item_id] > 0)
    }

    fn canonicalize_cycle_indices(cycle: &[usize]) -> Vec<usize> {
        // Rotates a cycle index list so equivalent cycles share one canonical representation.
        // This prevents duplicate cycles discovered from different DFS start points.
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

    fn depth_first_collect_cycles(
        start: usize,
        current: usize,
        adjacency: &[Vec<usize>],
        on_path: &mut [bool],
        path: &mut Vec<usize>,
        seen_cycles: &mut HashSet<Vec<usize>>,
        cycles: &mut Vec<Vec<usize>>,
    ) {
        // Explores graph paths from `start` and records simple cycles that return to `start`.
        // Uses on-path tracking to avoid revisiting nodes already in the active DFS branch.
        for &next in &adjacency[current] {
            if next == start && path.len() > 1 {
                let cycle = canonicalize_cycle_indices(path);
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
            depth_first_collect_cycles(start, next, adjacency, on_path, path, seen_cycles, cycles);
            path.pop();
            on_path[next] = false;
        }
    }

    let mut adjacency = vec![Vec::new(); recipes.len()];
    for (from_index, from_recipe) in recipes.iter().enumerate() {
        for (to_index, to_recipe) in recipes.iter().enumerate() {
            if recipe_outputs_feed_recipe_inputs(from_recipe, to_recipe) {
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
        depth_first_collect_cycles(
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
