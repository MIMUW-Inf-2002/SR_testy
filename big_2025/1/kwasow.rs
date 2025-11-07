use assignment_1_solution::{Handler, Module, ModuleRef, System};
use ntest::timeout;
use std::cmp::{max, min};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::runtime::Handle;
/* ================ MODULES ================ */

// ====== CLOCK MODULE
#[derive(Clone)]
struct TimeTick {
    time: u64,
}

/// Simple module to emit ticks at set intervals and measure
/// drift.
#[derive(Clone)]
struct ClockModule {
    last_sys_time: SystemTime,
    max_drift: Arc<AtomicU64>,
    fire_counter: Arc<AtomicU64>,
}

impl ClockModule {
    fn new() -> ClockModule {
        ClockModule {
            last_sys_time: SystemTime::now(),
            max_drift: Arc::new(AtomicU64::new(0)),
            fire_counter: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Handler<TimeTick> for ClockModule {
    async fn handle(&mut self, msg: TimeTick) {
        let current_time = SystemTime::now();
        let time_diff = current_time.duration_since(self.last_sys_time).unwrap();

        let expected_time = msg.time;
        let real_time = time_diff.as_millis() as u64;

        let drift = max(expected_time, real_time) - min(expected_time, real_time);
        self.max_drift.fetch_max(drift, Ordering::SeqCst);
        self.fire_counter.fetch_add(1, Ordering::SeqCst);
        self.last_sys_time = current_time;
    }
}

// ====== SLEEP MODULE

#[derive(Clone)]
struct SleepTask {
    time: u64,
}

/// Simple module that sleeps for the time indicated by the message
#[derive(Clone)]
struct SleepModule {
    times_slept: Arc<AtomicU64>,
}

impl SleepModule {
    fn new() -> SleepModule {
        SleepModule {
            times_slept: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Handler<SleepTask> for SleepModule {
    async fn handle(&mut self, msg: SleepTask) {
        tokio::time::sleep(Duration::from_millis(msg.time)).await;

        self.times_slept.fetch_add(1, Ordering::SeqCst);
    }
}

/* ============== END MODULES ============== */

/* =========== SIMPLE CLOCK TEST =========== */
/* First timer message arrives after timeout and
 * the timer stops executing when stop() is called
 * on handler.
 */

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(5000)]
async fn kwasow_timer_drift_test() {
    let (mut system, clock, clock_ref) = setup_system(ClockModule::new()).await;
    let timer_ref = clock_ref
        .request_tick(TimeTick { time: 100 }, Duration::from_millis(100))
        .await;

    tokio::time::sleep(Duration::from_secs(2)).await;
    timer_ref.stop().await;
    system.shutdown().await;

    let max_drift = clock.max_drift.load(Ordering::SeqCst);
    let counter = clock.fire_counter.load(Ordering::SeqCst);

    assert_eq!(counter, 19);
    // Some implementation might drift more, but try to find an implementation that fits in these
    // limits.
    assert!(max_drift < 5);

    tokio::time::sleep(Duration::from_millis(100)).await;
    verify_workers_done();
}

/* ========= END SIMPLE CLOCK TEST ========= */

/* ============ SHUTDOWN 1 TEST ============ */
/* Shutdown should only process started messages
 * and not take anymore from the queue
 */

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(3000)]
async fn kwasow_shutdown_test_1() {
    let (mut system, sleep, sleep_ref) = setup_system(SleepModule::new()).await;

    let task = SleepTask { time: 500 };

    for _ in 0..10 {
        sleep_ref.send(task.clone()).await;
    }

    tokio::time::sleep(Duration::from_millis(1100)).await;
    system.shutdown().await;

    let sleep_counter = sleep.times_slept.load(Ordering::SeqCst);
    assert_eq!(sleep_counter, 3);
    verify_workers_done();
}

/* ========== END SHUTDOWN 1 TEST ========== */

/* ============ SHUTDOWN 2 TEST ============ */
/* Shutdown when timer is running doesn't panic
 * the system
 */

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(3000)]
async fn kwasow_shutdown_test_2() {
    let (mut system, sleep, sleep_ref) = setup_system(SleepModule::new()).await;

    let task = SleepTask { time: 500 };

    sleep_ref
        .request_tick(task, Duration::from_millis(50))
        .await;

    tokio::time::sleep(Duration::from_millis(1100)).await;
    system.shutdown().await;

    let sleep_counter = sleep.times_slept.load(Ordering::SeqCst);
    assert_eq!(sleep_counter, 3);
    tokio::time::sleep(Duration::from_millis(75)).await;
    verify_workers_done();
}

/* ========== END SHUTDOWN 2 TEST ========== */

/* ============ SHUTDOWN 3 TEST ============ */
/* Sending message to shut down system may panic
 * or may not, but the message shouldn't be
 * processed.
 */

// == Risky test, but should pass for a correct implementation
#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "ignore")]
#[timeout(500)]
async fn kwasow_shutdown_test_3() {
    let (mut system, sleep, sleep_ref) = setup_system(SleepModule::new()).await;

    let task = SleepTask { time: 50 };
    sleep_ref.send(task.clone()).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    system.shutdown().await;

    might_panic(async move {
        sleep_ref.send(task.clone()).await;
    })
    .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let sleep_counter = sleep.times_slept.load(Ordering::SeqCst);
    assert_eq!(sleep_counter, 1);
    verify_workers_done();
}

/* ========== END SHUTDOWN 3 TEST ========== */

/* ============ SHUTDOWN 4 TEST ============ */
/* Registering module to shut down system might panic,
 * but if it doesn't, the module shouldn't process
 * messages.
 */

// == Test introduces constraints not present in the task
// #[cfg(tokio_unstable)]
// #[tokio::test(unhandled_panic = "ignore")]
// async fn kwasow_shutdown_4_test() {
//     let mut system = System::new().await;
//
//     system.shutdown().await;
//
//     let sleep_mod = SleepModule::new();
//     let sleep_mod_clone = sleep_mod.clone();
//     let res = might_panic(async move {
//         let mod_ref = system.register_module(|_| sleep_mod_clone).await;
//         (system, mod_ref)
//     })
//     .await;
//
//     if let Some((_, mod_ref)) = res {
//         might_panic(async move {
//             mod_ref.send(SleepTask { time: 1 }).await;
//         }).await;
//         tokio::time::sleep(Duration::from_millis(10)).await;
//     }
//
//     assert_eq!(sleep_mod.times_slept.load(Ordering::SeqCst), 0);
//     verify_workers_done();
// }

/* ========== END SHUTDOWN 4 TEST ========== */

/* ============ SHUTDOWN 5 TEST ============ */
/* Registering timer to shut down system might panic,
 * but if it doesn't, the timer shouldn't emit
 * messages. No tokio task should spawn (or it
 * should quit immediately at least).
 */

// == Test introduces constraints not present in the task
// #[cfg(tokio_unstable)]
// #[tokio::test(unhandled_panic = "ignore")]
// #[timeout(500)]
// async fn kwasow_shutdown_5_test() {
//     let mut system = System::new().await;
//     let sleep_mod = SleepModule::new();
//     let sleep_mod_clone = sleep_mod.clone();
//     let mod_ref = system.register_module(|_| sleep_mod_clone).await;
//
//     system.shutdown().await;
//
//     might_panic(async move {
//         mod_ref
//             .request_tick(SleepTask { time: 1 }, Duration::from_millis(10))
//             .await;
//     })
//     .await;
//     tokio::time::sleep(Duration::from_millis(20)).await;
//
//     assert_eq!(sleep_mod.times_slept.load(Ordering::SeqCst), 0);
//     verify_workers_done();
// }

/* ========== END SHUTDOWN 5 TEST ========== */

/* =============== IDLE TEST =============== */
/* An empty system shouldn't have any tokio tasks
 * running (no global queue handler).
 */

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(200)]
async fn kwasow_idle_test() {
    let mut system = System::new().await;

    verify_workers_done();
    system.shutdown().await;
    verify_workers_done();
}

/* ============= END IDLE TEST ============= */

/* ========= TIMER EFFICIENCY TEST ========= */

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(5000)]
async fn kwasow_timer_efficiency_test() {
    let (mut system, sleep_mod, sleep_ref) = setup_system(SleepModule::new()).await;

    let message = SleepTask { time: 0 };

    for _ in 0..10 {
        sleep_ref
            .request_tick(message.clone(), Duration::from_secs(1))
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    system.shutdown().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert!(sleep_mod.times_slept.load(Ordering::SeqCst) >= 20);
    assert!(sleep_mod.times_slept.load(Ordering::SeqCst) <= 22);
    verify_workers_done();
}

/* ======= END TIMER EFFICIENCY TEST ======= */

/* ============= COLLATZ TESTS ============= */
/* Test the collatz conjecture, similarly to the
 * small assignment.
 */

struct ComputeMessage {
    value: u64,
    n: u64,
}

struct SetDivModuleMessage {
    module: ModuleRef<DivideModule>,
}

struct ResultMessage {
    result: u64,
}

struct MultiplyModule {
    div_mod: Option<ModuleRef<DivideModule>>,
    res_mod: ModuleRef<ResultModule>,
}

#[async_trait::async_trait]
impl Handler<ComputeMessage> for MultiplyModule {
    async fn handle(&mut self, msg: ComputeMessage) {
        assert_eq!(msg.value % 2, 1);

        if msg.value == 1 {
            self.res_mod.send(ResultMessage { result: msg.n }).await;
        } else {
            self.div_mod
                .as_mut()
                .unwrap()
                .send(ComputeMessage {
                    value: 3 * msg.value + 1,
                    n: msg.n + 1,
                })
                .await;
        }
    }
}

#[async_trait::async_trait]
impl Handler<SetDivModuleMessage> for MultiplyModule {
    async fn handle(&mut self, msg: SetDivModuleMessage) {
        self.div_mod = Some(msg.module);
    }
}

struct DivideModule {
    self_ref: ModuleRef<DivideModule>,
    mul_mod: ModuleRef<MultiplyModule>,
}

#[async_trait::async_trait]
impl Handler<ComputeMessage> for DivideModule {
    async fn handle(&mut self, msg: ComputeMessage) {
        assert_eq!(msg.value % 2, 0);

        let message = ComputeMessage {
            value: msg.value / 2,
            n: msg.n + 1,
        };
        if message.value % 2 == 0 {
            self.self_ref.send(message).await;
        } else {
            self.mul_mod.send(message).await;
        }
    }
}

#[derive(Clone)]
struct ResultModule {
    result: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl Handler<ResultMessage> for ResultModule {
    async fn handle(&mut self, msg: ResultMessage) {
        self.result.store(msg.result, Ordering::SeqCst);
    }
}

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(500)]
async fn kwasow_collatz_1_test() {
    test_collatz(1_234_567, 112, true).await;
    verify_workers_done();
}

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(500)]
async fn kwasow_collatz_2_test() {
    test_collatz(1, 1, true).await;
    verify_workers_done();
}

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(500)]
async fn kwasow_collatz_3_test() {
    test_collatz(2048, 12, true).await;
    verify_workers_done();
}

#[cfg(tokio_unstable)]
#[tokio::test(unhandled_panic = "shutdown_runtime")]
#[timeout(500)]
async fn kwasow_collatz_shutdown_test() {
    // We need to collect system here so it doesn't go out of scope and drop
    let mut system = test_collatz(1_234_567, 112, false).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let metrics = Handle::current().metrics();

    assert!(metrics.num_alive_tasks() > 0);
    system.shutdown().await;
    verify_workers_done();
}

async fn test_collatz(n: u64, steps: u64, shutdown: bool) -> System {
    let (mut system, mul_ref, div_ref, res_mod) = setup_collatz().await;

    let init_msg = ComputeMessage { value: n, n: 1 };
    if n % 2 == 0 {
        div_ref.send(init_msg).await;
    } else {
        mul_ref.send(init_msg).await;
    }

    tokio::time::sleep(Duration::from_millis(200)).await;
    if shutdown {
        system.shutdown().await;
    }

    let steps_taken = res_mod.result.load(Ordering::SeqCst);
    assert_eq!(steps, steps_taken);

    system
}

async fn setup_collatz() -> (
    System,
    ModuleRef<MultiplyModule>,
    ModuleRef<DivideModule>,
    ResultModule,
) {
    let mut system = System::new().await;

    let res_mod = ResultModule {
        result: Arc::new(AtomicU64::new(0)),
    };
    let res_ref = system.register_module(|_| res_mod.clone()).await;
    let mul_ref = system
        .register_module(|_| MultiplyModule {
            div_mod: None,
            res_mod: res_ref,
        })
        .await;
    let div_ref = system
        .register_module(|s| DivideModule {
            self_ref: s,
            mul_mod: mul_ref.clone(),
        })
        .await;

    mul_ref
        .send(SetDivModuleMessage {
            module: div_ref.clone(),
        })
        .await;

    (system, mul_ref, div_ref, res_mod)
}

/* ========== END COLLATZ 3 TESTS ========== */

/* ============= HELPER METHODS ============ */

async fn setup_system<T: Module + Clone>(module: T) -> (System, T, ModuleRef<T>) {
    let mut system = System::new().await;
    let module_clone = module.clone();

    let module_ref = system.register_module(|_| module_clone).await;

    (system, module, module_ref)
}

fn verify_workers_done() {
    let metrics = Handle::current().metrics();

    assert_eq!(metrics.num_alive_tasks(), 0);
}

async fn might_panic<F>(future: F) -> Option<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let result = tokio::spawn(future).await;
    // We're making sure that if the coroutine finished with an error
    // then it is a panic. It is also acceptable to not throw any
    // errors for send() when a system is already shutdown
    match result {
        Ok(output) => Some(output),
        Err(err) => {
            assert!(err.is_panic());
            None
        }
    }
}

/* =========== END HELPER METHODS ========== */
