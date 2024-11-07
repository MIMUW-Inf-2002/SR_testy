use assignment_1_solution::{Handler, ModuleRef, System};
use ntest::timeout;
use std::borrow::BorrowMut;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

const ROUNDS: u32 = 5;

struct PingPong {
    other: Option<ModuleRef<PingPong>>,
    received_msgs: u32,
    first: bool,
    name: &'static str,
    log_sender: UnboundedSender<String>,
}

#[derive(Clone)]
struct Ball;

#[derive(Clone)]
struct Init {
    target: ModuleRef<PingPong>,
}

#[async_trait::async_trait]
impl Handler<Init> for PingPong {
    async fn handle(&mut self, _self_ref: &ModuleRef<Self>, msg: Init) {
        self.other = Some(msg.target);
        if self.first {
            self.other.as_ref().unwrap().send(Ball).await;
        }
    }
}

fn prepare_msg(name: &str, round: u32) -> String {
    format!("In {}: received {}\n", name, round)
}

#[async_trait::async_trait]
impl Handler<Ball> for PingPong {
    async fn handle(&mut self, _self_ref: &ModuleRef<Self>, _msg: Ball) {
        self.log_sender
            .send(prepare_msg(self.name, self.received_msgs))
            .unwrap();

        self.received_msgs += 1;
        if self.received_msgs < ROUNDS {
            self.other.as_ref().unwrap().send(Ball).await;
        }
    }
}

async fn initialize_system(sys: &mut System) -> UnboundedReceiver<String> {
    let (log_sender, log_receiver) = unbounded_channel();
    let ping = sys
        .register_module(PingPong {
            other: None,
            name: "Ping",
            received_msgs: 0,
            first: true,
            log_sender: log_sender.clone(),
        })
        .await;
    let pong = sys
        .register_module(PingPong {
            other: None,
            name: "Pong",
            received_msgs: 0,
            first: false,
            log_sender,
        })
        .await;

    pong.send(Init {
        target: ping.clone(),
    })
    .await;
    ping.send(Init { target: pong }).await;
    log_receiver
}

#[tokio::test]
#[timeout(300)]
async fn ping_pong_runs_correctly() {
    let mut sys = System::new().await;
    let mut log_receiver = initialize_system(sys.borrow_mut()).await;

    for round in 0..ROUNDS {
        let names = if round < ROUNDS - 1 {
            vec!["Pong", "Ping"]
        } else {
            vec!["Pong"]
        };
        for name in names {
            assert_eq!(prepare_msg(name, round), log_receiver.recv().await.unwrap());
        }
    }

    sys.shutdown().await;
}

#[derive(Clone)]
struct Tick;

struct Timer {
    first_tick_received: bool,
    timeout_callback: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl Timer {
    fn new(timeout_callback: Pin<Box<dyn Future<Output = ()> + Send>>) -> Self {
        Self {
            first_tick_received: false,
            timeout_callback: Some(timeout_callback),
        }
    }
}

#[async_trait::async_trait]
impl Handler<Tick> for Timer {
    async fn handle(&mut self, _self_ref: &ModuleRef<Self>, _msg: Tick) {
        if !self.first_tick_received {
            self.first_tick_received = true;
        } else {
            match self.timeout_callback.take() {
                Some(callback) => callback.await,
                None => (),
            }
        }
    }
}

struct Timeout;

async fn set_timer(
    system: &mut System,
    timeout_callback: Pin<Box<dyn Future<Output = ()> + Send>>,
    duration: Duration,
) -> ModuleRef<Timer> {
    let timer = system.register_module(Timer::new(timeout_callback)).await;
    timer.request_tick(Tick, duration).await;
    timer
}

#[tokio::test]
#[timeout(300)]
async fn second_tick_arrives_after_correct_interval() {
    let mut sys = System::new().await;
    let (timeout_sender, mut timeout_receiver) = unbounded_channel::<Timeout>();
    let timeout_interval = Duration::from_millis(50);

    let start_instant = Instant::now();
    set_timer(
        &mut sys,
        Box::pin(async move {
            timeout_sender.send(Timeout).unwrap();
        }),
        timeout_interval,
    )
    .await;
    timeout_receiver.recv().await.unwrap();
    let elapsed = start_instant.elapsed();

    assert!((elapsed.as_millis() as i128 - (timeout_interval.as_millis() * 2) as i128).abs() <= 2);
    sys.shutdown().await;
}

struct CountToFive {
    five_sender: UnboundedSender<u8>,
}

#[async_trait::async_trait]
impl Handler<u8> for CountToFive {
    async fn handle(&mut self, self_ref: &ModuleRef<Self>, msg: u8) {
        if msg == 5 {
            self.five_sender.send(msg).unwrap();
        } else {
            self_ref.send(msg + 1).await;
        }
    }
}

#[tokio::test]
#[timeout(300)]
async fn self_ref_works() {
    let mut system = System::new().await;
    let (five_sender, mut five_receiver) = unbounded_channel::<u8>();
    let count_to_five = system.register_module(CountToFive { five_sender }).await;

    count_to_five.send(1).await;

    assert_eq!(five_receiver.recv().await.unwrap(), 5);

    system.shutdown().await;
}

struct Counter {
    num: u8,
    num_sender: UnboundedSender<u8>,
}

#[async_trait::async_trait]
impl Handler<Tick> for Counter {
    async fn handle(&mut self, _self_ref: &ModuleRef<Self>, _msg: Tick) {
        self.num_sender.send(self.num).unwrap();
        self.num += 1;
    }
}

#[tokio::test]
#[timeout(500)]
async fn stopping_ticks_works() {
    let mut system = System::new().await;
    let (num_sender, mut num_receiver) = unbounded_channel();
    let counter_ref = system.register_module(Counter { num: 0, num_sender }).await;

    let timer_handle = counter_ref
        .request_tick(Tick, Duration::from_millis(50))
        .await;
    tokio::time::sleep(Duration::from_millis(170)).await;
    timer_handle.stop().await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut received_numbers = Vec::new();
    while let Ok(num) = num_receiver.try_recv() {
        received_numbers.push(num);
    }
    assert_eq!(received_numbers, vec![0, 1, 2]);

    system.shutdown().await;
}

#[tokio::test]
#[timeout(500)]
async fn multiple_ticks_works() {
    let mut system = System::new().await;
    let (num_sender, mut num_receiver) = unbounded_channel();
    let counter_ref_1 = system
        .register_module(Counter {
            num: 0,
            num_sender: num_sender.clone(),
        })
        .await;
    let counter_ref_2 = system
        .register_module(Counter {
            num: 0,
            num_sender: num_sender.clone(),
        })
        .await;

    let timer_handle_1 = counter_ref_1
        .request_tick(Tick, Duration::from_millis(50))
        .await;
    tokio::time::sleep(Duration::from_millis(70)).await; // until now it sent 1 tick

    let timer_handle_2 = counter_ref_2
        .request_tick(Tick, Duration::from_millis(100))
        .await;
    // now there are two working concurrently

    tokio::time::sleep(Duration::from_millis(120)).await;
    timer_handle_1.stop().await;
    // by this time 1 should have sent 3 ticks and stopped, 2 should have sent 1 tick

    tokio::time::sleep(Duration::from_millis(100)).await;
    timer_handle_2.stop().await;
    // overall 1 should have sent 3 ticks and 2 should have sent 2

    let mut received_numbers = Vec::new();
    while let Ok(num) = num_receiver.try_recv() {
        received_numbers.push(num);
    }
    assert_eq!(received_numbers.len(), 5);

    system.shutdown().await;
}

#[cfg(tokio_unstable)]
#[tokio::test(flavor = "current_thread", unhandled_panic = "shutdown_runtime")]
#[timeout(500)]
async fn tickers_dont_panic_when_shutdown() {
    let mut system = System::new().await;
    let (num_sender, mut _num_receiver) = unbounded_channel();
    let counter_ref = system
        .register_module(Counter {
            num: 0,
            num_sender: num_sender.clone(),
        })
        .await;

    counter_ref
        .request_tick(Tick, Duration::from_millis(50))
        .await;

    tokio::time::sleep(Duration::from_millis(70)).await;
    system.shutdown().await;
    // tick could panic if it finishes tick after shutdown
    // or if it keeps running after shutdown
    tokio::time::sleep(Duration::from_millis(100)).await; // until now it sent 1 tick
}

struct SleepyCounter {
    num: u8,
    sleep_in_millis: u64,
    num_sender: UnboundedSender<u8>,
}

#[async_trait::async_trait]
impl Handler<Tick> for SleepyCounter {
    async fn handle(&mut self, _self_ref: &ModuleRef<Self>, _msg: Tick) {
        tokio::time::sleep(Duration::from_millis(self.sleep_in_millis)).await;
        self.num_sender.send(self.num).unwrap();
        self.num += 1;
    }
}

#[cfg(tokio_unstable)]
#[test]
#[timeout(400)]
fn all_tasks_finish_after_shutdown() {
    // this test creates some modules, requests ticks a couple of times and manually sends some ticks
    // afterward the system is shut down. It fails if there are any unhandled panics or
    // there is some unterminated task
    let (task_spawn_sender, task_spawn_receiver) = unbounded_channel();
    let (task_kill_sender, task_kill_receiver) = unbounded_channel();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .unhandled_panic(tokio::runtime::UnhandledPanic::ShutdownRuntime)
        .on_task_spawn(move |_| {
            task_spawn_sender.send(true).unwrap();
        })
        .on_task_terminate(move |_| {
            task_kill_sender.send(true).unwrap();
        })
        .build()
        .unwrap();

    runtime.block_on(async {
        let mut system = System::new().await;
        let (num_sender, _num_receiver) = unbounded_channel();
        for i in 0..4 {
            let r = system
                .register_module(SleepyCounter {
                    num: 0,
                    num_sender: num_sender.clone(),
                    sleep_in_millis: 50,
                })
                .await;
            if i % 2 == 0 {
                r.request_tick(Tick, Duration::from_millis(10)).await;
                r.request_tick(Tick, Duration::from_millis(20)).await;
                r.request_tick(Tick, Duration::from_millis(30)).await;
            } else {
                r.send(Tick).await;
                r.send(Tick).await;
                r.send(Tick).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await; // just so something can happen
        system.shutdown().await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let spawned_tasks = task_spawn_receiver.len();
        let killed_tasks = task_kill_receiver.len();
        assert_eq!(spawned_tasks, killed_tasks);
    })
}

#[cfg(tokio_unstable)]
#[tokio::test(flavor = "current_thread", unhandled_panic = "shutdown_runtime")]
#[timeout(200)]
async fn started_handlers_finish_after_shutdown() {
    let mut system = System::new().await;
    let (num_sender, mut num_receiver) = unbounded_channel();

    let sleepy_ctr_1 = system
        .register_module(SleepyCounter {
            num: 0,
            num_sender: num_sender.clone(),
            sleep_in_millis: 50,
        })
        .await;
    let sleepy_ctr_2 = system
        .register_module(SleepyCounter {
            num: 0,
            num_sender: num_sender.clone(),
            sleep_in_millis: 70,
        })
        .await;

    for _ in 0..5 {
        sleepy_ctr_1.send(Tick).await;
        sleepy_ctr_2.send(Tick).await;
    }

    tokio::time::sleep(Duration::from_millis(100)).await; // both started handling second message
    system.shutdown().await;
    // now they both should have counted up to two and stopped
    let mut received_nums = Vec::new();
    while let Ok(num) = num_receiver.try_recv() {
        received_nums.push(num);
    }
    received_nums.sort();
    assert_eq!(received_nums, vec![0, 0, 1, 1]);
}

