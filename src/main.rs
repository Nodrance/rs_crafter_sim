mod crafting_domain;
mod crafting_solver;
mod demo_scenario;
mod execution_planner;
mod recipe_analysis;

use crafting_domain::item_display_name;
use demo_scenario::{
    build_demo_recipes, build_demo_starting_items, build_demo_target_items,
};
use crafting_solver::{
    compute_max_craftable_target_amount, compute_required_base_items,
    find_executable_solution_via_cycle_elimination,
};

fn main() {
    // Executes the full demo workflow: load scenario data, compute max craftable output,
    // search for an executable cycle-safe solution branch, and print either the
    // executable plan or fallback base-item deficit guidance.
    let recipes = build_demo_recipes();
    let starting_items = build_demo_starting_items();
    let target = build_demo_target_items();

    let max = compute_max_craftable_target_amount(recipes.clone(), starting_items.clone(), target.clone());
    println!("Maximum additional quantity of the primary target item: {}", max);
    if max == 0 {
        println!("No feasible crafting solution found for the current target and starting inventory.");
        let required_items = compute_required_base_items(recipes, starting_items, target);
        if required_items.items.is_empty() {
            println!("Relaxed deficit analysis found no additional non-producible items to add.");
        } else {
            println!("Add the following base items to the starting inventory:");
            for (item_id, count) in required_items.items {
                println!("- {}: {}", item_display_name(item_id), count);
            }
        }
        return;
    }

    let executable_or_fallback = find_executable_solution_via_cycle_elimination(
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

        println!("No executable plan was found after cycle-elimination branching; running base-item deficit analysis.");
        println!(
            "Deficit analysis will use {} recipes retained after branch filtering.",
            fallback_recipes.len()
        );
        let required_items = compute_required_base_items(fallback_recipes, starting_items, target);
        if required_items.items.is_empty() {
            println!("Relaxed deficit analysis found no additional non-producible items to add.");
        } else {
            println!("Add the following base items to the starting inventory:");
            for (item_id, count) in required_items.items {
                println!("- {}: {}", item_display_name(item_id), count);
            }
        }
        return;
    }

    let (solution, plan) = executable_or_fallback.unwrap();
    println!("Found an executable crafting solution for the target.");

    println!("\nSolved recipe usage totals:");
    for recipe in &recipes {
        let var_value = solution.recipe_usage_count(recipe);
        if var_value == 0.0 {
            continue;
        }
        println!("- {}: {}", recipe.describe(), var_value);
    }

    println!("\nExecutable recipe application plan:");
    for (recipe, count) in plan {
        println!("- Apply '{}' x{}", recipe.describe(), count);
    }

    println!("\nProjected final inventory:");
    for item in &solution.relevant_item_ids {
        let final_val = solution.final_inventory_count(*item);
        if final_val == 0.0 {
            continue;
        }
        println!("- {}: {}", item_display_name(*item), final_val);
    }
}
