use assignment_2_solution::{
    serialize_register_command, ClientCommandHeader, ClientRegisterCommand,
    ClientRegisterCommandContent, RegisterCommand, SectorVec, MAGIC_NUMBER,
};
use assignment_2_test_utils::system::{HmacSha256, HMAC_TAG_SIZE};
use hmac::Mac;
use ntest::timeout;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::Duration,
};

const EXPECTED_RESPONSES_SIZE: usize = 48;

#[tokio::test]
#[timeout(40000)]
#[ignore]
async fn external_write() {
    // given
    log_init();
    let hmac_client_key = [98; 32]; /* Same `b` jeśli dobrze kojarzę */
    let tcp_ports = [21626, 21627, 21628];
    let request_identifier = 1778;

    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", tcp_ports[0]))
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
    send_cmd(&write_cmd, &mut stream, &hmac_client_key).await;

    // then
    let mut buf = [0_u8; EXPECTED_RESPONSES_SIZE];
    stream
        .read_exact(&mut buf)
        .await
        .expect("Less data than expected");

    // asserts for write response
    assert_eq!(&buf[0..4], MAGIC_NUMBER.as_ref());
    assert_eq!(buf[7], 0x42);
    assert_eq!(
        u64::from_be_bytes(buf[8..16].try_into().unwrap()),
        request_identifier
    );
    assert!(hmac_tag_is_ok(&hmac_client_key, &buf));
}

/* Warning! This should be only used with external_write test beforehand */
/* While external_write is standalone test, this one is used to check restored data */
#[tokio::test]
#[timeout(40000)]
#[ignore]
async fn external_read() {
    // given
    log_init();
    let hmac_client_key = [98; 32]; /* Same `b` jeśli dobrze kojarzę */
    let tcp_ports = [21626, 21627, 21628];
    let request_identifier = 1778;

    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", tcp_ports[0]))
        .await
        .expect("Could not connect to TCP port");

    let read_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx: 12,
        },
        content: ClientRegisterCommandContent::Read,
    });

    // when
    send_cmd(&read_cmd, &mut stream, &hmac_client_key).await;

    // then
    let mut buf = [0_u8; EXPECTED_RESPONSES_SIZE + 4096];
    stream
        .read_exact(&mut buf)
        .await
        .expect("Less data than expected");

    // asserts for write response
    assert_eq!(&buf[0..4], MAGIC_NUMBER.as_ref());
    assert_eq!(buf[7], 0x41);
    assert_eq!(
        u64::from_be_bytes(buf[8..16].try_into().unwrap()),
        request_identifier
    );
    assert_eq!(
        buf[16..4112],    
        [3; 4096]
    );
    assert!(hmac_tag_is_ok(&hmac_client_key, &buf));
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
