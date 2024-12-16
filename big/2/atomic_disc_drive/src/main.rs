use tokio::fs::read_to_string;
use std::path::PathBuf;
use std::str::FromStr;
use assignment_2_solution::{run_register_process, Configuration, PublicConfiguration};

async fn read_config_from_file(fpath: PathBuf, self_rank: u8, storage_dir: PathBuf) -> Configuration {
    let system_hmac: [u8; 64];
    let client_hmac: [u8; 32];
    let mut tcp_locations: Vec<(String, u16)> = Vec::new();
    let contents = read_to_string(fpath).await.unwrap();

    let mut lines = contents.lines();
    system_hmac = lines.next().unwrap().as_bytes().try_into().unwrap();
    client_hmac = lines.next().unwrap().as_bytes().try_into().unwrap();

    let n_sectors = u64::from_str(lines.next().unwrap()).unwrap();

    while let Some(host) = lines.next() {
        let port = u16::from_str(lines.next().unwrap()).unwrap();
        tcp_locations.push((host.to_string(), port));
    }

    Configuration {
        hmac_system_key: system_hmac,
        hmac_client_key: client_hmac,
        public: PublicConfiguration {
            storage_dir,
            tcp_locations,
            self_rank,
            n_sectors,
        },
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 3)]
async fn main() {
    env_logger::init();
    let conf_path = std::env::args().nth(1).expect("Need a configuration file!");
    let self_rank = std::env::args().nth(2).expect("Need a self-id");
    let storage_dir = std::env::args().nth(3).expect("Need a storage directory");

    let self_rank = u8::from_str(&self_rank).unwrap();
    let storage_dir = PathBuf::from(storage_dir);
    let configuration = read_config_from_file(PathBuf::from(conf_path), self_rank, storage_dir).await;
    log::debug!("Loaded config: {:?}", configuration);
    run_register_process(configuration).await;
}