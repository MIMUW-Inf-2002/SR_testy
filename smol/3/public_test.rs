#[cfg(test)]
mod tests {
    use crate::solution::{FibonacciSystemMessage, FibonacciModule, fib, run_executor};
    use crossbeam_channel::unbounded;
    use ntest::timeout;

    #[test]
    #[timeout(200)]
    fn fib_ends() {
        fib(10);
    }

    #[test]
    fn create_registers_new_module() {
        let (tx, rx) = unbounded();
        FibonacciModule::create(0, 7, tx);

        assert_eq!(rx.len(), 1);
        match rx.try_recv().unwrap() {
            FibonacciSystemMessage::RegisterModule(_) => {}
            _ => panic!("Creating module resulted in a different message than RegisterModule!"),
        }
    }

    #[test]
    fn register_many_modules() {
        let n = 1000;
        let (tx, rx) = unbounded();
        for _ in 0..n { FibonacciModule::create(0, 7, tx.clone()); }


        assert_eq!(rx.len(), n);

        for _ in 0..n {
            match rx.try_recv().unwrap() {
                FibonacciSystemMessage::RegisterModule(_) => {}
                _ => panic!("Creating module resulted in a different message than RegisterModule!"),
            }
        }
    }

    #[test]
    fn many_fibs() {
        let n = 1000;
        let mut prev_id = None;
        let (tx, rx) = unbounded();

        for i in 0..n {
            let curr_id = FibonacciModule::create(i % 2, 7, tx.clone());
            if i % 2 == 1 {
                tx.send(FibonacciSystemMessage::Init { id: curr_id, other: prev_id.unwrap() }).unwrap();
                tx.send(FibonacciSystemMessage::Init { id: prev_id.unwrap(), other: curr_id }).unwrap();
            }
            prev_id = Some(curr_id);
        }


        assert_eq!(rx.len(), 2 * n as usize);

        run_executor(rx).join().unwrap();
    }

    #[test]
    fn talking_to_module_a() {
        let (tx, rx) = unbounded();
        FibonacciModule::create(0, 7, tx.clone());

        assert_eq!(rx.len(), 1);
        let mut module = match rx.try_recv().unwrap() {
            FibonacciSystemMessage::RegisterModule(m) => m,
            _ => panic!("Creating module resulted in a different message than RegisterModule!"),
        };

        module.init(0);
        assert_eq!(rx.len(), 0);

        module.message(1, 123);
        assert_eq!(rx.len(), 1);

        match rx.try_recv().unwrap() {
            FibonacciSystemMessage::Message { id: 0, idx: 2, num: 123 } => {}
            _ => panic!("Bad Message response!"),
        };
    }

    #[test]
    fn talking_to_module_b() {
        let (tx, rx) = unbounded();
        FibonacciModule::create(1, 7, tx.clone());

        assert_eq!(rx.len(), 1);
        let mut module = match rx.try_recv().unwrap() {
            FibonacciSystemMessage::RegisterModule(m) => m,
            _ => panic!("Creating module resulted in a different message than RegisterModule!"),
        };

        module.init(0);
        assert_eq!(rx.len(), 1);

        // B should send Init message
        match rx.try_recv().unwrap() {
            FibonacciSystemMessage::Message { id: 0, idx:1, num:1 } => {}
            _ => panic!("Bad Init response!"),
        };

        module.message(1, 123);
        assert_eq!(rx.len(), 1);

        match rx.try_recv().unwrap() {
            FibonacciSystemMessage::Message { id: 0, idx: 2, num: 124 } => {}
            _ => panic!("Bad Message response!"),
        };
    }
}
