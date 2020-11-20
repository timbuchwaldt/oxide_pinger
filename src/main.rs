#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate notify;

use config::*;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use oping::{Ping, PingResult};
use prometheus_exporter::prometheus::register_counter_vec;
use prometheus_exporter::prometheus::register_histogram_vec;
use prometheus_exporter::prometheus::CounterVec;
use prometheus_exporter::prometheus::HistogramVec;
use std::process;
use std::sync::mpsc::channel;
use std::sync::RwLock;
use std::time::Duration;
use std::{thread, time};

lazy_static! {
    static ref LOST_COUNTS: CounterVec =
        register_counter_vec!("icmp_timeout", "help", &["host"]).unwrap();
    static ref SUCCESSFUL_COUNTS: CounterVec =
        register_counter_vec!("icmp_successful", "help", &["host"]).unwrap();
    static ref HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "icmp_response",
        "ICMP Response timie",
        &["host"],
        vec![0.0, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 200.0]
    )
    .unwrap();
    static ref SETTINGS: RwLock<Config> = RwLock::new({
        let mut settings = Config::default();
        settings.merge(File::with_name("settings.toml")).unwrap();
        settings.set_default("listener", "127.0.0.1:9184").unwrap();
        settings.set_default("hosts", vec!["127.0.0.1"]).unwrap();
        settings.set_default("icmp_interval", 1000).unwrap();
        settings.set_default("icmp_timeout", 0.5).unwrap();
        settings
    });
}

fn do_pings(hosts: Vec<config::Value>) -> PingResult<()> {
    let mut ping = Ping::new();
    ping.set_timeout(SETTINGS.read().unwrap().get_float("icmp_interval").unwrap())?;

    for h in hosts {
        debug!("Adding host {}", h.to_string());
        ping.add_host(&h.to_string())?;
    }

    let maybe_responses = ping.send();
    match maybe_responses {
        Ok(responses) => {
            for resp in responses {
                if resp.dropped > 0 {
                    LOST_COUNTS.with_label_values(&[&resp.hostname]).inc();
                // println!("No response from host: {}", resp.hostname);
                } else {
                    HISTOGRAM_VEC
                        .with_label_values(&[&resp.hostname])
                        .observe(resp.latency_ms);

                    SUCCESSFUL_COUNTS.with_label_values(&[&resp.hostname]).inc();
                    /* println!(
                        "Response from host {} (address {}): latency {} ms",
                        resp.hostname, resp.address, resp.latency_ms
                    ); */
                    // println!("    all details: {:?}", resp);
                }
            }
        }
        Err(_) => {
            println!("Bla");
        }
    }

    Ok(())
}

fn main() {
    env_logger::init();

    match SETTINGS
        .read()
        .unwrap()
        .get_str("listener")
        .unwrap()
        .parse()
    {
        Ok(s) => match prometheus_exporter::start(s) {
            Ok(_) => {}
            Err(_) => {
                process::exit(1);
            }
        },
        Err(_) => {
            process::exit(1);
        }
    }
    std::thread::spawn(|| {
        watch();
    });

    loop {
        let desired_sleep_time = time::Duration::from_millis(
            SETTINGS.read().unwrap().get_int("icmp_interval").unwrap() as u64,
        );

        let start_time = time::Instant::now();
        match do_pings(SETTINGS.read().unwrap().get_array(&"hosts").unwrap()) {
            Ok(_) => {}
            Err(_) => {
                process::exit(1);
            }
        }
        let now = time::Instant::now();
        let wait_time = now - start_time;
        debug!("Ran ping, took: {:?}", wait_time);
        if wait_time < desired_sleep_time {
            let sleep_time = desired_sleep_time - wait_time;
            debug!("Sleeping for, {:?}", sleep_time);

            thread::sleep(sleep_time);
        } else {
            warn!("Ping took longer than desired loop interval")
        }
    }
}

fn watch() {
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2)).unwrap();

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch("./settings.toml", RecursiveMode::NonRecursive)
        .unwrap();

    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(DebouncedEvent::Write(_)) => {
                debug!("settings.toml written; refreshing configuration ...");
                SETTINGS.write().unwrap().refresh().unwrap();
            }

            Err(e) => println!("watch error: {:?}", e),

            _ => {
                // Ignore event
            }
        }
    }
}
