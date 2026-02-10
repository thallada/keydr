#[allow(dead_code)]
pub fn polynomial_regression(times: &[f64]) -> Option<f64> {
    if times.len() < 3 {
        return None;
    }

    let n = times.len();
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();

    let x_mean: f64 = xs.iter().sum::<f64>() / n as f64;
    let y_mean: f64 = times.iter().sum::<f64>() / n as f64;

    let mut ss_xy = 0.0;
    let mut ss_xx = 0.0;
    let mut ss_yy = 0.0;

    for i in 0..n {
        let dx = xs[i] - x_mean;
        let dy = times[i] - y_mean;
        ss_xy += dx * dy;
        ss_xx += dx * dx;
        ss_yy += dy * dy;
    }

    if ss_xx < 1e-10 || ss_yy < 1e-10 {
        return None;
    }

    let slope = ss_xy / ss_xx;
    let r_squared = (ss_xy * ss_xy) / (ss_xx * ss_yy);

    if r_squared < 0.5 {
        return None;
    }

    let predicted_next = y_mean + slope * (n as f64 - x_mean);
    Some(predicted_next.max(0.0))
}

#[allow(dead_code)]
pub fn learning_rate_description(times: &[f64]) -> &'static str {
    match polynomial_regression(times) {
        Some(predicted) => {
            if times.is_empty() {
                return "No data";
            }
            let current = times.last().unwrap();
            let improvement = (current - predicted) / current * 100.0;
            if improvement > 5.0 {
                "Improving"
            } else if improvement < -5.0 {
                "Slowing down"
            } else {
                "Steady"
            }
        }
        None => "Not enough data",
    }
}
