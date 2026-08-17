use serde_json::Value;

pub(crate) fn metric_loss(metrics: &Value, objective: &Value, complexity_penalty: f64) -> f64 {
    objective["silhouette_iou"].as_f64().unwrap_or(0.0)
        * (1.0 - metrics["silhouette_iou"].as_f64().unwrap_or(0.0))
        + objective["boundary_f1_4px"].as_f64().unwrap_or(0.0)
            * (1.0 - metrics["boundary_f1_4px"].as_f64().unwrap_or(0.0))
        + objective["landmark_coverage"].as_f64().unwrap_or(0.0)
            * (1.0 - metrics["landmark_coverage"].as_f64().unwrap_or(0.0))
        + objective["landmark_nme"].as_f64().unwrap_or(0.0)
            * metrics["landmark_nme"].as_f64().unwrap_or(0.0)
        + objective["part_region"].as_f64().unwrap_or(0.0)
            * metrics["part_region_error"].as_f64().unwrap_or(0.0)
        + objective["program_complexity"].as_f64().unwrap_or(0.0) * complexity_penalty
}

pub(crate) fn weighted_loss(losses: impl IntoIterator<Item = (f64, f64)>) -> f64 {
    let mut total = 0.0;
    let mut weight = 0.0;
    for (loss, item_weight) in losses {
        total += loss * item_weight;
        weight += item_weight;
    }
    if weight > 0.0 {
        total / weight
    } else {
        f64::INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_loss_is_order_independent_and_bounded() {
        let forward = weighted_loss([(0.2, 1.0), (0.8, 3.0)]);
        let reverse = weighted_loss([(0.8, 3.0), (0.2, 1.0)]);
        assert!((forward - 0.65).abs() < 1.0e-12);
        assert_eq!(forward, reverse);
    }

    #[test]
    fn empty_weighted_loss_is_not_promotable() {
        assert!(weighted_loss(std::iter::empty::<(f64, f64)>()).is_infinite());
    }
}
