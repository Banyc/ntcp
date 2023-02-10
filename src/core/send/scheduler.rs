use std::{collections::HashMap, hash::Hash};

pub struct Scheduler<K> {
    weight_vector: HashMap<K, f64>,
    learning_rate: f64,
}

impl<K> Scheduler<K>
where
    K: Eq + Hash + Copy,
{
    #[must_use]
    pub fn new_empty(learning_rate: f64) -> Self {
        Self {
            weight_vector: HashMap::new(),
            learning_rate,
        }
    }

    #[must_use]
    pub fn new(fd_vector: impl Iterator<Item = K>, learning_rate: f64) -> Self {
        let mut this = Self {
            weight_vector: HashMap::new(),
            learning_rate,
        };

        // Init weight vector
        this.init_weight(fd_vector);

        this
    }

    fn init_weight(&mut self, fds: impl Iterator<Item = K>) {
        for key in fds {
            self.weight_vector.insert(key, 1.0);
        }
        let even_weight = 1.0 / self.weight_vector.len() as f64;
        for weight in self.weight_vector.values_mut() {
            *weight = even_weight;
        }
    }

    /// Do not include RTTs that are either infinite, NaN, or time out.
    pub fn update(&mut self, rtt_vector: &HashMap<K, f64>) {
        if self.weight_vector.len() == 0 {
            // Init weight vector
            self.init_weight(rtt_vector.keys().copied());
        }

        let clean_rtt_vector = normalize(rtt_vector);

        // Get minimum RTT index
        let Some(min_rtt_index) = arg_min_key(clean_rtt_vector.iter()) else {
            // `rtt_vector` is empty
            return;
        };

        // To remove dead fds from the next weight vector
        let mut next_weight_vector = HashMap::new();

        // Update weight vector
        for (key, rtt) in clean_rtt_vector.iter() {
            // Get current weight
            let weight = self.weight(key).unwrap();

            // Calculate partial derivative
            let partial_derivative = match key == min_rtt_index {
                true => -*rtt,
                false => *rtt,
            };

            // Nudge the weight in the opposite direction of the gradient
            let mut next_weight = weight - self.learning_rate * partial_derivative;

            // Prevent negative weight
            if next_weight < 0.0 {
                next_weight = 0.0;
            }

            // Store next weight
            next_weight_vector.insert(*key, next_weight);
        }

        // Normalize weight vector
        normalize_mut(&mut next_weight_vector);

        // Store weight vector
        self.weight_vector = next_weight_vector;
    }

    #[must_use]
    pub fn weight(&self, key: &K) -> Option<f64> {
        if self.weight_vector.len() == 0 {
            // A valid weight vector cannot be empty
            return None;
        }
        let weight = match self.weight_vector.get(key) {
            Some(weight) => *weight,
            None => 0.0, // New FD
        };
        Some(weight)
    }
}

#[must_use]
#[allow(dead_code)]
fn normalize<K>(vector: &HashMap<K, f64>) -> HashMap<K, f64>
where
    K: Eq + Hash + Copy,
{
    let mut normalized_vector = HashMap::new();
    let sum: f64 = vector.values().sum();
    for (key, weight) in vector {
        normalized_vector.insert(*key, *weight / sum);
    }
    normalized_vector
}

#[must_use]
#[allow(dead_code)]
fn standardize<K>(vector: &HashMap<K, f64>) -> Result<HashMap<K, f64>, StandardizeError>
where
    K: Eq + Hash + Copy,
{
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
    for (key, weight) in vector {
        standardized_vector.insert(*key, (*weight - mean) / std_dev);
    }
    Ok(standardized_vector)
}

enum StandardizeError {
    ZeroStdDev,
    TooFewSamples,
}

fn normalize_mut<K>(vector: &mut HashMap<K, f64>) {
    let sum: f64 = vector.values().sum();
    for weight in vector.values_mut() {
        *weight /= sum;
    }
}

#[must_use]
fn arg_min_key<'a, K, I>(vector: I) -> Option<&'a K>
where
    I: Iterator<Item = (&'a K, &'a f64)>,
{
    let mut min_key = None;
    let mut min_value = f64::MAX;
    for (key, value) in vector {
        if *value < min_value {
            min_value = *value;
            min_key = Some(key);
        }
    }
    min_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut scheduler = Scheduler::new(vec![0, 1, 2].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert_eq!(scheduler.weight(&0).unwrap(), 1.0 / 3.0);
        assert_eq!(scheduler.weight(&1).unwrap(), 1.0 / 3.0);
        assert_eq!(scheduler.weight(&2).unwrap(), 1.0 / 3.0);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight(&0).unwrap() > prev_weight_vector[&0]);
        assert!(scheduler.weight(&1).unwrap() < prev_weight_vector[&1]);
        assert!(scheduler.weight(&2).unwrap() < prev_weight_vector[&2]);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("2nd: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight(&0).unwrap() > prev_weight_vector[&0]);
        assert!(scheduler.weight(&1).unwrap() < prev_weight_vector[&1]);
        assert!(scheduler.weight(&2).unwrap() < prev_weight_vector[&2]);

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
        assert!(scheduler.weight(&0).unwrap() > 0.999);
        assert!(scheduler.weight(&1).unwrap() < 0.001);
        assert!(scheduler.weight(&2).unwrap() < 0.001);

        let prev_weight_vector = scheduler.weight_vector.clone();

        // RTTs swap
        scheduler.update(
            &vec![(0, 300.0), (1, 200.0), (2, 100.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("103th: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight(&0).unwrap() < prev_weight_vector[&0]);
        assert!(f64::abs(scheduler.weight(&1).unwrap() - prev_weight_vector[&1]) < 0.001);
        assert!(scheduler.weight(&2).unwrap() > prev_weight_vector[&2]);
    }

    #[test]
    fn fd_removal() {
        let mut scheduler = Scheduler::new(vec![0, 1, 2].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert_eq!(scheduler.weight(&0).unwrap(), 1.0 / 3.0);
        assert_eq!(scheduler.weight(&1).unwrap(), 1.0 / 3.0);
        assert_eq!(scheduler.weight(&2).unwrap(), 1.0 / 3.0);

        // Update weight vector
        scheduler.update(&vec![(0, 100.0), (1, 200.0)].into_iter().collect());
        assert_eq!(scheduler.weight_vector.len(), 2);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight(&0).unwrap() > 0.5);
        assert!(scheduler.weight(&1).unwrap() < 0.5);
    }

    #[test]
    fn fd_addition() {
        let mut scheduler = Scheduler::new(vec![0, 1].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 2);
        assert_eq!(scheduler.weight(&0).unwrap(), 1.0 / 2.0);
        assert_eq!(scheduler.weight(&1).unwrap(), 1.0 / 2.0);

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 300.0), (2, 200.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        println!("1st: {:?}", scheduler.weight_vector);
        assert!(scheduler.weight(&0).unwrap() > 1.0 / 3.0);
        assert!(scheduler.weight(&1).unwrap() > scheduler.weight(&2).unwrap());
        assert_eq!(scheduler.weight(&2).unwrap(), 0.0);
    }

    #[test]
    fn from_empty_ok() {
        let mut scheduler = Scheduler::new_empty(0.1);
        assert_eq!(scheduler.weight_vector.len(), 0);

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert!(scheduler.weight(&0).unwrap() > 1.0 / 3.0);
        assert!(scheduler.weight(&1).unwrap() < 1.0 / 3.0);
        assert!(scheduler.weight(&2).unwrap() < 1.0 / 3.0);
    }
}
