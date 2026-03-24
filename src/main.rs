use std::time::Instant;

use rs_crafter_sim::model::item_display_name;
use rs_crafter_sim::demo_scenarios::{
    build_demo_scenario, build_stress_scenario,
};
use rs_crafter_sim::crafting_solver::{
    compute_max_craftable_target_amount, compute_required_base_items,
    find_executable_solution_via_cycle_elimination,
};

#[allow(dead_code)]
enum Scenario {
    Demo,
    Stress,
}

fn print_required_base_items_report(required_items: rs_crafter_sim::model::ItemSet) {
    // Prints out what items you need to add to the starting inventory to make the target craftable, based on the output of compute_required_base_items.
    if required_items.items.is_empty() {
        println!("Relaxed deficit analysis found no additional non-producible items to add.");
    } else {
        println!("Add the following base items to the starting inventory:");
        for (item_id, count) in required_items.items {
            println!("- {}: {}", item_display_name(item_id), count);
        }
    }
}

fn run() {
    // Loads a demo based on the SCENARIO variable. Some of them work and some overwhelm the program
    // Runs it ITERATIONS times for timing measurement
    // Starts by checking how many of the item can be crafted
    // Then tries to make a plan for crafting it (in the form of "use this recipe this many times")
    // If it can't be crafted, it'll try to figure out what items you need to add to the starting inventory to make it craftable, and print that out instead.
    const SCENARIO: Scenario = Scenario::Demo;
    const ITERATIONS: usize = 1;
    rs_crafter_sim::debugln!(
        "[debug] Run starting with scenario={:?}, iterations={}, debug={}.",
        match SCENARIO {
            Scenario::Demo => "Demo",
            Scenario::Stress => "Stress",
        },
        ITERATIONS,
        rs_crafter_sim::DEBUG_LOGGING_ENABLED
    );

    let (recipes, starting_items, target) = match SCENARIO {
        Scenario::Demo => build_demo_scenario(),
        Scenario::Stress => build_stress_scenario(),
    };

    rs_crafter_sim::debugln!(
        "[debug] Scenario loaded: recipes={}, starting-items={}, target-items={}",
        recipes.len(),
        starting_items.items.len(),
        target.items.len()
    );

    for iteration in 0..ITERATIONS {
        let is_last_iteration = iteration + 1 == ITERATIONS;
        rs_crafter_sim::debugln!("[debug] Solver iteration {} / {}", iteration + 1, ITERATIONS);

        let max = compute_max_craftable_target_amount(recipes.clone(), starting_items.clone(), target.clone());
        println!("Maximum additional quantity of the primary target item: {}", max);
        if max == 0 {
            println!("No feasible crafting solution found for the current target and starting inventory.");
            let required_items = compute_required_base_items(recipes, starting_items, target);
            print_required_base_items_report(required_items);
            return;
        }

        let executable_or_fallback = find_executable_solution_via_cycle_elimination(
            recipes.clone(),
            starting_items.clone(),
            target.clone(),
        );

        if executable_or_fallback.is_err() {
            if !is_last_iteration {
                continue;
            }

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
            let required_items = compute_required_base_items(
                fallback_recipes,
                starting_items.clone(),
                target.clone(),
            );
            print_required_base_items_report(required_items);
            continue;
        }

        if !is_last_iteration {
            continue;
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
}

fn main() {
    let started_at = Instant::now();
    run();
    println!("Execution time: {:.3?}", started_at.elapsed());
}
