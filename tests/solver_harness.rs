use std::{fs, path::PathBuf};

use rs_crafter_sim::{
    model::{ItemSet, Recipe},
    crafting_solver::find_executable_solution_via_cycle_elimination,
};
use serde::Deserialize;

const FLOAT_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Deserialize)]
struct SolverHarnessFile {
    cases: Vec<SolverCase>,
}

#[derive(Debug, Deserialize)]
struct SolverCase {
    name: String,
    description: String,
    recipes: Vec<RecipeSpec>,
    starting_items: Vec<(usize, usize)>,
    target: Vec<(usize, usize)>,
    expect: Expectation,
}

#[derive(Debug, Deserialize)]
struct RecipeSpec {
    input: Vec<(usize, usize)>,
    output: Vec<(usize, usize)>,
    priority: isize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Expectation {
    Ok {
        remaining_inventory: Vec<(usize, usize)>,
        recipe_invocation_counts: Vec<usize>,
    },
    Error,
}

#[test]
fn solver_harness_runs_cases_from_file_sequentially() {
    let harness_data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("cases");

    let mut harness_data_paths = fs::read_dir(&harness_data_dir)
        .unwrap_or_else(|err| {
            panic!(
                "Failed to read solver harness directory at {}: {err}",
                harness_data_dir.display()
            )
        })
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;
            let path = entry.path();
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("json") => Some(path),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    harness_data_paths.sort_unstable();

    assert!(
        !harness_data_paths.is_empty(),
        "No harness case files found in {}",
        harness_data_dir.display()
    );

    let mut global_case_index = 0usize;
    for harness_data_path in harness_data_paths {
        let harness_raw = fs::read_to_string(&harness_data_path).unwrap_or_else(|err| {
            panic!(
                "Failed to read solver harness file at {}: {err}",
                harness_data_path.display()
            )
        });

        let harness_file: SolverHarnessFile = serde_json::from_str(&harness_raw).unwrap_or_else(|err| {
            panic!(
                "Failed to parse solver harness JSON at {}: {err}",
                harness_data_path.display()
            )
        });

        for case in harness_file.cases {
            global_case_index += 1;

            let recipes = case
                .recipes
                .iter()
                .map(|recipe| {
                    Recipe::from_transform(recipe.input.clone(), recipe.output.clone(), recipe.priority)
                })
                .collect::<Vec<_>>();
            let starting_items = ItemSet::from_item_counts(case.starting_items.clone());
            let target = ItemSet::from_item_counts(case.target.clone());

            println!(
                "Running case #{}: '{}' from file {}. Description: {}",
                global_case_index,
                case.name,
                harness_data_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
                case.description
            );
            let result = find_executable_solution_via_cycle_elimination(
                recipes.clone(),
                starting_items,
                target,
            );
            println!(
                "Calculation complete"
            );

            match case.expect {
                Expectation::Ok {
                    remaining_inventory,
                    recipe_invocation_counts,
                } => {
                    let case_result = result.unwrap_or_else(|disabled_recipe_ids| {
                        panic!(
                            "Case #{} ('{}') expected success but returned error with disabled recipe ids: {:?}. Description: {}",
                            global_case_index,
                            case.name,
                            disabled_recipe_ids,
                            case.description
                        )
                    });

                    let (solution, _) = case_result;

                    assert_eq!(
                        recipe_invocation_counts.len(),
                        recipes.len(),
                        "Case #{} ('{}') must provide expected recipe invocation count for each recipe in order. Description: {}",
                        global_case_index,
                        case.name,
                        case.description
                    );

                    for (recipe_index, expected_count) in recipe_invocation_counts.iter().enumerate() {
                        let actual = solution.recipe_usage_count(&recipes[recipe_index]);
                        assert!(
                            (actual - *expected_count as f64).abs() <= FLOAT_TOLERANCE,
                            "Case #{} ('{}') recipe index {} expected count {} but got {}. Description: {}",
                            global_case_index,
                            case.name,
                            recipe_index,
                            expected_count,
                            actual,
                            case.description
                        );
                    }

                    for (item_id, expected_remaining) in remaining_inventory {
                        let actual = solution.final_inventory_count(item_id);
                        assert!(
                            (actual - expected_remaining as f64).abs() <= FLOAT_TOLERANCE,
                            "Case #{} ('{}') item {} expected remaining {} but got {}. Description: {}",
                            global_case_index,
                            case.name,
                            item_id,
                            expected_remaining,
                            actual,
                            case.description
                        );
                    }
                }
                Expectation::Error => {
                    assert!(
                        result.is_err(),
                        "Case #{} ('{}') expected error status but solver returned a solution. Description: {}",
                        global_case_index,
                        case.name,
                        case.description
                    );
                }
            }
        }
    }
}
