#[cfg(test)]
mod tests {
    use crate::solution::Threadpool;
    use crossbeam_channel::unbounded;
    use ntest::timeout;
    use std::sync::Arc;

    #[test]
    #[timeout(200)]
    fn smoke_test() {
        let (tx, rx) = unbounded();
        let pool = Threadpool::new(1);

        pool.submit(Box::new(move || {
            tx.send(14).unwrap();
        }));

        assert_eq!(14, rx.recv().unwrap());
    }

    #[test]
    #[timeout(200)]
    fn threadpool_is_sync() {
        let send_only_when_threadpool_is_sync = Arc::new(Threadpool::new(1));
        let (tx, rx) = unbounded();

        let _handle = std::thread::spawn(move || {
            tx.send(send_only_when_threadpool_is_sync).unwrap();
        });

        rx.recv().unwrap();
    }

    #[test]
    #[timeout(1200)]
    fn threadpool_big() {
        let n = 100;
        let size = 10;
        let (tx, rx) = unbounded();
        let pool = Threadpool::new(size);
        let sleep_time = 1000 / n * size;

        for i in 0..n {
            let tx = tx.clone();
            pool.submit(Box::new(move || {
                std::thread::sleep(std::time::Duration::from_millis(sleep_time as u64));
                tx.send(i).unwrap();
            }));
        }

        let mut received = Vec::new();
        for _ in 0..n {
            received.push(rx.recv().unwrap());
        }
        received.sort();
        (0..n).for_each(|i| {
            assert_eq!(i, received[i]);
        });
    }

    #[test]
    #[timeout(1080)]
    #[repeat(20)]
    fn test_all_at_once() {
        let thread_count: usize = 64;
        let sleep_for: u64 = 1000;
        let pool = Threadpool::new(thread_count);

        for _ in 0..thread_count {
            pool.submit(Box::new(move || {
                std::thread::sleep(std::time::Duration::from_millis(sleep_for));
            }));
        }
    }
}
