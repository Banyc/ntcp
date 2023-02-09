use std::{collections::HashMap, os::fd::RawFd};

pub struct Scheduler {
    weight_vector: HashMap<RawFd, f64>,
    learning_rate: f64,
}

impl Scheduler {
    pub fn new(fd_vector: impl Iterator<Item = RawFd>, learning_rate: f64) -> Self {
        // Init weight vector
        let mut weight_vector = HashMap::new();
        for fd in fd_vector {
            weight_vector.insert(fd, 1.0);
        }
        let even_weight = 1.0 / weight_vector.len() as f64;
        for weight in weight_vector.values_mut() {
            *weight = even_weight;
        }

        Self {
            weight_vector,
            learning_rate,
        }
    }

    /// Do not include RTTs that are either infinite, NaN, or time out.
    pub fn update(&mut self, rtt_vector: &HashMap<RawFd, f64>) {
        // Standardize RTTs to N(0, 1)
        let Ok(clean_rtt_vector) = &standardize(rtt_vector) else {
            // If there is no valid RTT, do nothing
            return;
        };

        // To remove dead fds from the next weight vector
        let mut next_weight_vector = HashMap::new();

        // Update weight vector
        for (fd, rtt) in clean_rtt_vector.iter() {
            // Get current weight
            let weight = match self.weight_vector.get(fd) {
                Some(weight) => *weight,
                None => {
                    // This is a new fd
                    0.0
                }
            };

            // Nudge the weight in the opposite direction of the gradient
            let mut next_weight = weight - self.learning_rate * rtt;

            // Prevent negative weight
            if next_weight < 0.0 {
                next_weight = 0.0;
            }

            // Store next weight
            next_weight_vector.insert(*fd, next_weight);
        }

        // Normalize weight vector
        normalize_mut(&mut next_weight_vector);

        // Store weight vector
        self.weight_vector = next_weight_vector;
    }
}

#[must_use]
#[allow(dead_code)]
fn normalize(vector: &HashMap<RawFd, f64>) -> HashMap<RawFd, f64> {
    let mut normalized_vector = HashMap::new();
    let sum: f64 = vector.values().sum();
    for (fd, weight) in vector {
        normalized_vector.insert(*fd, *weight / sum);
    }
    normalized_vector
}

#[must_use]
fn standardize(vector: &HashMap<RawFd, f64>) -> Result<HashMap<RawFd, f64>, StandardizeError> {
    if vector.len() < 2 {
        return Err(StandardizeError::TooFewSamples);
    }
    let mut standardized_vector = HashMap::new();
    let mean: f64 = vector.values().sum::<f64>() / vector.len() as f64;
    let mut sum_of_squares = 0.0;
    for weight in vector.values() {
        sum_of_squares += (weight - mean).powi(2);
    }
    if sum_of_squares == 0.0 {
        return Err(StandardizeError::ZeroStdDev);
    }
    let std_dev = (sum_of_squares / (vector.len() - 1) as f64).sqrt();
    for (fd, weight) in vector {
        standardized_vector.insert(*fd, (*weight - mean) / std_dev);
    }
    Ok(standardized_vector)
}

enum StandardizeError {
    ZeroStdDev,
    TooFewSamples,
}

fn normalize_mut(vector: &mut HashMap<RawFd, f64>) {
    let sum: f64 = vector.values().sum();
    for weight in vector.values_mut() {
        *weight /= sum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut scheduler = Scheduler::new(vec![0, 1, 2].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert_eq!(scheduler.weight_vector[&0], 1.0 / 3.0);
        assert_eq!(scheduler.weight_vector[&1], 1.0 / 3.0);
        assert_eq!(scheduler.weight_vector[&2], 1.0 / 3.0);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] > prev_weight_vector[&0]);
        assert!(f64::abs(scheduler.weight_vector[&1] - prev_weight_vector[&1]) < 0.001);
        assert!(scheduler.weight_vector[&2] < prev_weight_vector[&2]);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("2nd: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] > prev_weight_vector[&0]);
        assert!(f64::abs(scheduler.weight_vector[&1] - prev_weight_vector[&1]) < 0.001);
        assert!(scheduler.weight_vector[&2] < prev_weight_vector[&2]);

        let _prev_weight_vector = scheduler.weight_vector.clone();

        // Converge final weight vector
        for _ in 0..100 {
            scheduler.update(
                &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                    .into_iter()
                    .collect(),
            );
        }
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("102th: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] > 0.999);
        assert!(scheduler.weight_vector[&1] < 0.001);
        assert!(scheduler.weight_vector[&2] < 0.001);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // RTTs swap
        scheduler.update(
            &vec![(0, 300.0), (1, 200.0), (2, 100.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("103th: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] < prev_weight_vector[&0]);
        assert!(f64::abs(scheduler.weight_vector[&1] - prev_weight_vector[&1]) < 0.001);
        assert!(scheduler.weight_vector[&2] > prev_weight_vector[&2]);
    }

    #[test]
    fn fd_removal() {
        let mut scheduler = Scheduler::new(vec![0, 1, 2].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert_eq!(scheduler.weight_vector[&0], 1.0 / 3.0);
        assert_eq!(scheduler.weight_vector[&1], 1.0 / 3.0);
        assert_eq!(scheduler.weight_vector[&2], 1.0 / 3.0);

        // Update weight vector
        scheduler.update(&vec![(0, 100.0), (1, 200.0)].into_iter().collect());
        assert_eq!(scheduler.weight_vector.len(), 2);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] > 0.5);
        assert!(scheduler.weight_vector[&1] < 0.5);
    }

    #[test]
    fn fd_addition() {
        let mut scheduler = Scheduler::new(vec![0, 1].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 2);
        assert_eq!(scheduler.weight_vector[&0], 1.0 / 2.0);
        assert_eq!(scheduler.weight_vector[&1], 1.0 / 2.0);

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 300.0), (2, 200.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight_vector[&0] > 1.0 / 3.0);
        assert!(scheduler.weight_vector[&1] > scheduler.weight_vector[&2]);
        assert_eq!(scheduler.weight_vector[&2], 0.0);
    }
}
