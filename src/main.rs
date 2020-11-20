#[macro_use]
extern crate lazy_static;
use oping::{Ping, PingResult};
use std::fmt;

use prometheus_exporter::prometheus::register_counter_vec;
use prometheus_exporter::prometheus::register_histogram_vec;
use prometheus_exporter::prometheus::CounterVec;
use prometheus_exporter::prometheus::HistogramVec;

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
}

fn do_pings() -> PingResult<()> {
    let mut ping = Ping::new();
    ping.set_timeout(1.0); // timeout of 5.0 seconds
    ping.add_host("localhost"); // fails here if socket can't be created
    ping.add_host("8.8.8.8");
    ping.add_host("::1"); // IPv4 / IPv6 addresses OK
    ping.add_host("1.1.1.1");
    ping.add_host("9.9.9.9");

    for i in 1..255 {
        ping.add_host(&format!("1.1.1.{}", i));
    }

    let maybe_responses = ping.send();
    match maybe_responses {
        Ok(responses) => {
            for resp in responses {
                if resp.dropped > 0 {
                    LOST_COUNTS.with_label_values(&[&resp.hostname]).inc();
                    println!("No response from host: {}", resp.hostname);
                } else {
                    HISTOGRAM_VEC
                        .with_label_values(&[&resp.hostname])
                        .observe(resp.latency_ms);

                    SUCCESSFUL_COUNTS.with_label_values(&[&resp.hostname]).inc();
                    println!(
                        "Response from host {} (address {}): latency {} ms",
                        resp.hostname, resp.address, resp.latency_ms
                    );
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
    prometheus_exporter::start("0.0.0.0:9184".parse().unwrap());

    loop {
        do_pings();
    }
}
