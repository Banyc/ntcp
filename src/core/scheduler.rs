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

    pub fn update(&mut self, rtt_vector: &HashMap<RawFd, f64>) {
        // Update weight vector
        let normalized_rtt_vector = normalize(rtt_vector);
        for (fd, weight) in self.weight_vector.iter_mut() {
            *weight -= self.learning_rate * normalized_rtt_vector[fd];
        }
        normalize_mut(&mut self.weight_vector);
    }
}

#[must_use]
fn normalize(vector: &HashMap<RawFd, f64>) -> HashMap<RawFd, f64> {
    let mut normalized_vector = HashMap::new();
    let sum: f64 = vector.values().sum();
    for (fd, weight) in vector {
        normalized_vector.insert(*fd, *weight / sum);
    }
    normalized_vector
}

fn normalize_mut(vector: &mut HashMap<RawFd, f64>) {
    let sum: f64 = vector.values().sum();
    for weight in vector.values_mut() {
        *weight /= sum;
    }
}

mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut scheduler = Scheduler::new(vec![0, 1, 2].into_iter(), 0.1);
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert_eq!(scheduler.weight_vector[&0], 0.3333333333333333);
        assert_eq!(scheduler.weight_vector[&1], 0.3333333333333333);
        assert_eq!(scheduler.weight_vector[&2], 0.3333333333333333);

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert!(scheduler.weight_vector[&0] < 0.352);
        assert!(scheduler.weight_vector[&0] > 0.351);
        assert!(scheduler.weight_vector[&1] < 0.334);
        assert!(scheduler.weight_vector[&1] > 0.333);
        assert!(scheduler.weight_vector[&2] < 0.315);
        assert!(scheduler.weight_vector[&2] > 0.314);

        // Update weight vector
        scheduler.update(
            &vec![(0, 100.0), (1, 200.0), (2, 300.0)]
                .into_iter()
                .collect(),
        );
        assert_eq!(scheduler.weight_vector.len(), 3);
        assert!(scheduler.weight_vector[&0] < 0.373);
        assert!(scheduler.weight_vector[&0] > 0.372);
        assert!(scheduler.weight_vector[&1] < 0.334);
        assert!(scheduler.weight_vector[&1] > 0.333);
        assert!(scheduler.weight_vector[&2] < 0.295);
        assert!(scheduler.weight_vector[&2] > 0.294);
    }
}