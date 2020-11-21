#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate notify;

use config::*;
use fastping_rs::PingResult::{Idle, Receive};
use fastping_rs::Pinger;
use ipnet::IpNet;
use prometheus_exporter::prometheus::register_counter_vec;
use prometheus_exporter::prometheus::register_histogram_vec;
use prometheus_exporter::prometheus::CounterVec;
use prometheus_exporter::prometheus::HistogramVec;
use std::process;
use std::sync::RwLock;

lazy_static! {
    static ref LOST_COUNTS: CounterVec =
        register_counter_vec!("icmp_timeout", "help", &["host"]).unwrap();
    static ref HISTOGRAM_VEC: HistogramVec = register_histogram_vec!(
        "icmp_response",
        "ICMP Response time",
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

fn do_pings(hosts: Vec<config::Value>) {
    let (pinger, results) = match Pinger::new(None, None) {
        Ok((pinger, results)) => (pinger, results),
        Err(e) => panic!("Error creating pinger: {}", e),
    };

    for h in hosts {
        debug!("Adding network {}", h.to_string());
        let net: IpNet = h.to_string().parse().unwrap();

        for host in net.hosts() {
            debug!("Adding host {}", &host);
            pinger.add_ipaddr(&host.to_string());
        }
    }

    pinger.run_pinger();

    debug!("Sending pings");
    loop {
        match results.recv() {
            Ok(result) => match result {
                Idle { addr } => {
                    info!("Idle Address {}.", addr);
                    LOST_COUNTS.with_label_values(&[&addr.to_string()]).inc();
                }
                Receive { addr, rtt } => {
                    info!("Receive from Address {} in {:?}.", addr, rtt);
                    HISTOGRAM_VEC
                        .with_label_values(&[&addr.to_string()])
                        .observe(rtt.as_millis() as f64);
                }
            },
            Err(_) => panic!("Worker threads disconnected before the solution was found!"),
        }
    }
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

    do_pings(SETTINGS.read().unwrap().get_array(&"hosts").unwrap())
}
