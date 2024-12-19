use assignment_2_solution::deserialize_register_command;
use assignment_2_solution::run_register_process;
use assignment_2_solution::serialize_register_command;
use assignment_2_solution::ClientCommandHeader;
use assignment_2_solution::ClientRegisterCommand;
use assignment_2_solution::ClientRegisterCommandContent;
use assignment_2_solution::Configuration;
use assignment_2_solution::PublicConfiguration;
use assignment_2_solution::RegisterCommand;
use assignment_2_solution::SectorVec;
use assignment_2_solution::SystemRegisterCommandContent;
use assignment_2_solution::MAGIC_NUMBER;
use assignment_2_test_utils::system::HmacSha256;
use assignment_2_test_utils::system::RegisterResponseContent;
use assignment_2_test_utils::system::TestProcessesConfig;
use assignment_2_test_utils::system::HMAC_TAG_SIZE;
use hmac::Mac;
use ntest::timeout;
use std::collections::HashMap;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::Duration;

#[tokio::test]
#[timeout(4000)]
async fn bad_hmac() {
    // given
    log_init();
    let hmac_client_key = [5; 32];
    let tcp_port = 2311;
    let storage_dir = tempdir().unwrap();
    let request_identifier = 1778;

    let config = Configuration {
        public: PublicConfiguration {
            tcp_locations: vec![("127.0.0.1".to_string(), tcp_port)],
            self_rank: 1,
            n_sectors: 20,
            storage_dir: storage_dir.into_path(),
        },
        hmac_system_key: [1; 64],
        hmac_client_key,
    };

    tokio::spawn(run_register_process(config));

    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", tcp_port))
        .await
        .expect("Could not connect to TCP port");
    let write_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx: 12,
        },
        content: ClientRegisterCommandContent::Write {
            data: SectorVec(vec![3; 4096]),
        },
    });

    // when
    send_cmd(&write_cmd, &mut stream, &[4; 32]).await;

    // then
    const EXPECTED_RESPONSES_SIZE: usize = 48;
    let mut buf = [0_u8; EXPECTED_RESPONSES_SIZE];
    stream
        .read_exact(&mut buf)
        .await
        .expect("Less data then expected");

    // asserts for write response
    assert_eq!(&buf[0..4], MAGIC_NUMBER.as_ref());
    assert_eq!(buf[6], 0x1); /* Invalid hmac */
    assert_eq!(buf[7], 0x42);
    assert_eq!(
        u64::from_be_bytes(buf[8..16].try_into().unwrap()),
        request_identifier
    );
    assert!(hmac_tag_is_ok(&hmac_client_key, &buf));
}

#[tokio::test]
#[timeout(4000)]
async fn bad_sector_number() {
    // given
    log_init();
    let hmac_client_key = [5; 32];
    let tcp_port = 30_287;
    let storage_dir = tempdir().unwrap();
    let request_identifier = 1778;

    let config = Configuration {
        public: PublicConfiguration {
            tcp_locations: vec![("127.0.0.1".to_string(), tcp_port)],
            self_rank: 1,
            n_sectors: 20,
            storage_dir: storage_dir.into_path(),
        },
        hmac_system_key: [1; 64],
        hmac_client_key,
    };

    tokio::spawn(run_register_process(config));

    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", tcp_port))
        .await
        .expect("Could not connect to TCP port");
    let write_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx: 20,
        },
        content: ClientRegisterCommandContent::Write {
            data: SectorVec(vec![3; 4096]),
        },
    });

    // when
    send_cmd(&write_cmd, &mut stream, &hmac_client_key).await;

    // then
    const EXPECTED_RESPONSES_SIZE: usize = 48;
    let mut buf = [0_u8; EXPECTED_RESPONSES_SIZE];
    stream
        .read_exact(&mut buf)
        .await
        .expect("Less data then expected");

    // asserts for write response
    assert_eq!(&buf[0..4], MAGIC_NUMBER.as_ref());
    assert_eq!(buf[6], 0x2);
    assert_eq!(buf[7], 0x42);
    assert_eq!(
        u64::from_be_bytes(buf[8..16].try_into().unwrap()),
        request_identifier
    );
    assert!(hmac_tag_is_ok(&hmac_client_key, &buf));
}

#[tokio::test]
#[serial_test::serial]
#[timeout(30000)]
async fn concurrent_writes_are_serialized() {
    // given
    log_init();
    let port_range_start = 21518;
    let n_clients = 16;
    /* Spawn two and add our stub to system */
    let config = TestProcessesConfig::new(3, port_range_start);
    tokio::spawn(run_register_process(config.config(0)));
    tokio::spawn(run_register_process(config.config(1)));
    let listener = TcpListener::bind(config.tcp_locations[2].clone())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let (mut receiver, _addr) = listener.accept().await.unwrap();

    let mut streams = Vec::new();
    for _ in 0..n_clients {
        streams.push(config.connect(0).await);
    }
    // when
    for (i, stream) in streams.iter_mut().enumerate() {
        config
            .send_cmd(
                &RegisterCommand::Client(ClientRegisterCommand {
                    header: ClientCommandHeader {
                        request_identifier: i.try_into().unwrap(),
                        sector_idx: 0,
                    },
                    content: ClientRegisterCommandContent::Write {
                        data: SectorVec(vec![if i % 2 == 0 { 1 } else { 254 }; 4096]),
                    },
                }),
                stream,
            )
            .await;
    }

    for stream in &mut streams {
        config.read_response(stream).await.unwrap();
    }

    config
        .send_cmd(
            &RegisterCommand::Client(ClientRegisterCommand {
                header: ClientCommandHeader {
                    request_identifier: n_clients,
                    sector_idx: 0,
                },
                content: ClientRegisterCommandContent::Read,
            }),
            &mut streams[0],
        )
        .await;
    let response = config.read_response(&mut streams[0]).await.unwrap();

    match response.content {
        RegisterResponseContent::Read(SectorVec(sector)) => {
            assert!(sector == vec![1; 4096] || sector == vec![254; 4096]);
        }
        RegisterResponseContent::Write => panic!("Expected read response"),
    }

    // then

    let hmac_system_key: [u8; 64] = config.hmac_system_key.clone().try_into().unwrap();
    let mut data_written: HashMap<u64, SectorVec> = HashMap::new();
    loop {
        let (message, _) = deserialize_register_command(
            &mut receiver,
            &hmac_system_key,
            &config.hmac_client_key.clone().try_into().unwrap(),
        )
        .await
        .unwrap();
        let RegisterCommand::System(cmd) = message else {
            continue;
        };
        let SystemRegisterCommandContent::WriteProc {
            timestamp,
            write_rank: _,
            data_to_write,
        } = cmd.content
        else {
            continue;
        };

        if let Some(val) = data_written.get(&timestamp) {
            assert_eq!(val.0, data_to_write.0);
        }

        data_written.insert(timestamp, data_to_write);
        if timestamp >= n_clients {
            break;
        }
    }
}

fn log_init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

async fn send_cmd(register_cmd: &RegisterCommand, stream: &mut TcpStream, hmac_client_key: &[u8]) {
    let mut data = Vec::new();
    serialize_register_command(register_cmd, &mut data, hmac_client_key)
        .await
        .unwrap();

    stream.write_all(&data).await.unwrap();
}

fn hmac_tag_is_ok(key: &[u8], data: &[u8]) -> bool {
    let boundary = data.len() - HMAC_TAG_SIZE;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(&data[..boundary]);
    mac.verify_slice(&data[boundary..]).is_ok()
}
