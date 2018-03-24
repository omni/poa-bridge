use std::collections::VecDeque;
pub fn batch<I: Iterator, F, T>(iter: I, f: F, batch_size: usize) -> VecDeque<T>
    where F: Fn(Vec<I::Item>) -> T {
	let mut batches: VecDeque<T> = VecDeque::new();
	let mut batch = Vec::with_capacity(batch_size);
	let mut len = 0;
	for deposit in iter {
		len += 1;
		batch.push(deposit);
		if batch.len() == batch_size {
			batches.push_back(f(batch));
			batch = Vec::with_capacity(batch_size);
		}
	}
	if batch.len() > 0 || len == 0 {
		batches.push_back(f(batch));
	}
	batches
}
