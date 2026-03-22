use highs::RowProblem;

fn main() {
    let mut problem = RowProblem::new();
    
    // Add an integer variable bounded between 0 and i32::MAX
    let x = problem.add_integer_variable(0.0, i32::MAX as f64, "x");
    
    // Maximize x
    problem.set_objective_sense(highs::Sense::Maximize);
    problem.add_objective_coefficient(0, 1.0);
    
    // Solve
    let solution = problem.solve();
    
    println!("Status: {:?}", solution.status());
    println!("Objective value: {}", solution.objective_value());
    println!("x = {}", solution.variable(x));
}