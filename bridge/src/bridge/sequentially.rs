use futures::Future;
pub fn sequentially<F: Future, I: IntoIterator<Item = Box<F>>>(iter: I) -> Box<Future<Item = F::Item, Error = F::Error>>
    where F::Item: 'static, F::Error: 'static, F: 'static {
	let mut iter = iter.into_iter();
	let first = iter.next().unwrap();
	iter.fold(first, |acc, next| Box::new(acc.and_then(|_| next)))
}
