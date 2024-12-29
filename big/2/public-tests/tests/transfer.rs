use assignment_2_solution::{
    deserialize_register_command, serialize_register_command, ClientCommandHeader,
    ClientRegisterCommand, ClientRegisterCommandContent, RegisterCommand, SectorVec,
    SystemCommandHeader, SystemRegisterCommand, SystemRegisterCommandContent, MAGIC_NUMBER,
};
use assignment_2_test_utils::transfer::*;
use ntest::timeout;
use uuid::Uuid;

#[tokio::test]
#[timeout(200)]
async fn serialize_deserialize_is_identity() {
    // given
    let request_identifier = 7;
    let sector_idx = 8;
    let register_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx,
        },
        content: ClientRegisterCommandContent::Read,
    });
    let mut sink: Vec<u8> = Vec::new();

    // when
    serialize_register_command(&register_cmd, &mut sink, &[0x00_u8; 32])
        .await
        .expect("Could not serialize?");
    let mut slice: &[u8] = &sink[..];
    let data_read: &mut (dyn tokio::io::AsyncRead + Send + Unpin) = &mut slice;
    let (deserialized_cmd, hmac_valid) =
        deserialize_register_command(data_read, &[0x00_u8; 64], &[0x00_u8; 32])
            .await
            .expect("Could not deserialize");

    // then
    assert!(hmac_valid);
    match deserialized_cmd {
        RegisterCommand::Client(ClientRegisterCommand {
            header,
            content: ClientRegisterCommandContent::Read,
        }) => {
            assert_eq!(header.sector_idx, sector_idx);
            assert_eq!(header.request_identifier, request_identifier);
        }
        _ => panic!("Expected Read command"),
    }
}

#[tokio::test]
#[timeout(200)]
async fn serialize_deserialize_is_identity_write() {
    // given
    let request_identifier = 69;
    let sector_idx = 420;
    let data = SectorVec((0..4096).map(|_| rand::random::<u8>()).collect());
    let register_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx,
        },
        content: ClientRegisterCommandContent::Write { data: data.clone() },
    });
    let mut sink: Vec<u8> = Vec::new();

    // when
    serialize_register_command(&register_cmd, &mut sink, &[0x00_u8; 32])
        .await
        .expect("Could not serialize?");
    let mut slice: &[u8] = &sink[..];
    let data_read: &mut (dyn tokio::io::AsyncRead + Send + Unpin) = &mut slice;
    let (deserialized_cmd, hmac_valid) =
        deserialize_register_command(data_read, &[0x00_u8; 64], &[0x00_u8; 32])
            .await
            .expect("Could not deserialize");

    // then
    assert!(hmac_valid);

    match deserialized_cmd {
        RegisterCommand::Client(ClientRegisterCommand {
            header,
            content:
                ClientRegisterCommandContent::Write {
                    data: deserialized_data,
                },
        }) => {
            assert_eq!(header.sector_idx, sector_idx);
            assert_eq!(header.request_identifier, request_identifier);
            assert_eq!(data, deserialized_data);
        }
        _ => panic!("Expected Write command"),
    }
}

#[tokio::test]
#[timeout(200)]
async fn serialize_deserialize_is_identity_read_proc() {
    // given
    let sector_idx = 4525787855454_u64;
    let process_identifier = 147_u8;
    let msg_ident = [7; 16];

    let read_proc_cmd = RegisterCommand::System(SystemRegisterCommand {
        header: SystemCommandHeader {
            process_identifier,
            msg_ident: Uuid::from_slice(&msg_ident).unwrap(),
            sector_idx,
        },
        content: SystemRegisterCommandContent::ReadProc,
    });
    let mut sink: Vec<u8> = Vec::new();

    // when
    serialize_register_command(&read_proc_cmd, &mut sink, &[0x00_u8; 64])
        .await
        .expect("Could not serialize?");
    let mut slice: &[u8] = &sink[..];
    let data_read: &mut (dyn tokio::io::AsyncRead + Send + Unpin) = &mut slice;
    let (deserialized_cmd, hmac_valid) =
        deserialize_register_command(data_read, &[0x00_u8; 64], &[0x00_u8; 32])
            .await
            .expect("Could not deserialize");

    // then
    assert!(hmac_valid);

    match deserialized_cmd {
        RegisterCommand::System(SystemRegisterCommand {
            header,
            content: SystemRegisterCommandContent::ReadProc,
        }) => {
            assert_eq!(header.sector_idx, sector_idx);
            assert_eq!(header.process_identifier, process_identifier);
            assert_eq!(header.msg_ident.as_bytes().to_vec(), msg_ident.to_vec());
        }
        _ => panic!("Expected ReadProc command"),
    }
}

#[tokio::test]
#[timeout(200)]
async fn serialize_deserialize_is_identity_write_proc() {
    // given
    let sector_idx = 4525787855454_u64;
    let process_identifier = 147_u8;
    let msg_ident = [7; 16];
    let timestamp = 123456789_u64;
    let write_rank = 42_u8;
    let data_to_write = SectorVec((0..4096).map(|_| rand::random::<u8>()).collect());

    let write_proc_cmd = RegisterCommand::System(SystemRegisterCommand {
        header: SystemCommandHeader {
            process_identifier,
            msg_ident: Uuid::from_slice(&msg_ident).unwrap(),
            sector_idx,
        },
        content: SystemRegisterCommandContent::WriteProc {
            timestamp,
            write_rank,
            data_to_write: data_to_write.clone(),
        },
    });

    let mut sink: Vec<u8> = Vec::new();

    // when
    serialize_register_command(&write_proc_cmd, &mut sink, &[0x00_u8; 64])
        .await
        .expect("Could not serialize?");
    let mut slice: &[u8] = &sink[..];
    let data_read: &mut (dyn tokio::io::AsyncRead + Send + Unpin) = &mut slice;

    let (deserialized_cmd, hmac_valid) =
        deserialize_register_command(data_read, &[0x00_u8; 64], &[0x00_u8; 32])
            .await
            .expect("Could not deserialize");

    // then
    assert!(hmac_valid);

    match deserialized_cmd {
        RegisterCommand::System(SystemRegisterCommand {
            header,
            content:
                SystemRegisterCommandContent::WriteProc {
                    timestamp: deserialized_timestamp,
                    write_rank: deserialized_write_rank,
                    data_to_write: deserialized_data_to_write,
                },
        }) => {
            assert_eq!(header.sector_idx, sector_idx);
            assert_eq!(header.process_identifier, process_identifier);
            assert_eq!(header.msg_ident.as_bytes().to_vec(), msg_ident.to_vec());
            assert_eq!(timestamp, deserialized_timestamp);
            assert_eq!(write_rank, deserialized_write_rank);
            assert_eq!(data_to_write, deserialized_data_to_write);
        }
        _ => panic!("Expected WriteProc command"),
    }
}

#[tokio::test]
#[timeout(200)]
async fn serialized_read_proc_cmd_has_correct_format() {
    // given
    let sector_idx = 4525787855454_u64;
    let process_identifier = 147_u8;
    let msg_ident = [7; 16];

    let read_proc_cmd = RegisterCommand::System(SystemRegisterCommand {
        header: SystemCommandHeader {
            process_identifier,
            msg_ident: Uuid::from_slice(&msg_ident).unwrap(),
            sector_idx,
        },
        content: SystemRegisterCommandContent::ReadProc,
    });
    let mut serialized: Vec<u8> = Vec::new();

    // when
    serialize_register_command(&read_proc_cmd, &mut serialized, &[0x00_u8; 64])
        .await
        .expect("Could not write to vector?");
    serialized.truncate(serialized.len() - 32);

    // then
    assert_eq!(serialized.len(), 32);
    assert_system_cmd_header(
        serialized.as_slice(),
        &msg_ident,
        process_identifier,
        3,
        sector_idx,
    );
}

#[tokio::test]
#[timeout(200)]
async fn test_serialization_errors() {
    // given
    let request_identifier = 7;
    let sector_idx = 8;
    let register_cmd = RegisterCommand::Client(ClientRegisterCommand {
        header: ClientCommandHeader {
            request_identifier,
            sector_idx,
        },
        content: ClientRegisterCommandContent::Read,
    });
    // push some garbage to sink
    let mut garbage = vec![0x00_u8; 32];
    // append MAGIC_NUMBER
    garbage.append(&mut MAGIC_NUMBER.to_vec());
    garbage.append(&mut vec![0x00_u8, 0x00_u8, 0x00_u8, 0x69_u8]);
    garbage.append(&mut vec![0x00_u8; 3]);

    // when
    serialize_register_command(&register_cmd, &mut garbage, &[0x00_u8; 32])
        .await
        .expect("Could not serialize?");
    let mut slice: &[u8] = &garbage[..];
    let data_read: &mut (dyn tokio::io::AsyncRead + Send + Unpin) = &mut slice;

    let (deserialized_cmd, hmac_valid) =
        deserialize_register_command(data_read, &[0x00_u8; 64], &[0x00_u8; 32])
            .await
            .expect("Could not deserialize");

    // then
    assert!(hmac_valid);
    match deserialized_cmd {
        RegisterCommand::Client(ClientRegisterCommand {
            header,
            content: ClientRegisterCommandContent::Read,
        }) => {
            assert_eq!(header.sector_idx, sector_idx);
            assert_eq!(header.request_identifier, request_identifier);
        }
        _ => panic!("Expected Read command"),
    }
}
