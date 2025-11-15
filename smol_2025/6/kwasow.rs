#[cfg(test)]
mod tests {
    use crate::solution::build_stable_storage;
    use ntest::timeout;
    use tempfile::tempdir;

    #[tokio::test]
    #[timeout(500)]
    async fn remove_missing_key() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let res = storage.remove("key").await;
        assert_eq!(res, false);
    }

    #[tokio::test]
    #[timeout(500)]
    async fn get_missing_key() {
        let root_storage_dir = tempdir().unwrap();
        let storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let res = storage.get("key").await;
        assert_eq!(res, None);
    }

    #[tokio::test]
    #[timeout(500)]
    async fn store_invalid_key() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        // This string is 256 ASCII characters long (so 256 bytes)
        let invalid_key = "1234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678123456781234567812345678";
        let data = [0; 0];

        let res = storage.put(invalid_key, &data).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[timeout(500)]
    async fn store_invalid_data() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let key = "key";
        let invalid_data = [0u8; 65536];

        let res = storage.put(key, &invalid_data).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[timeout(500)]
    async fn blank_key_is_valid() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let key = "";
        let data = [0u8; 16];

        let res = storage.put(key, &data).await;
        assert!(res.is_ok());

        let res = storage.get(key).await;
        assert_eq!(res, Some(vec![0u8; 16]));
    }

    #[tokio::test]
    #[timeout(500)]
    async fn blank_data_is_valid() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let key = "key";
        let data = [0u8; 0];

        let res = storage.put(key, &data).await;
        assert!(res.is_ok());

        let res = storage.get(key).await;
        assert_eq!(res, Some(vec![]))
    }

    #[tokio::test]
    #[timeout(500)]
    async fn key_forbidden_character_1() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let key = "to/jest/wredna/nazwa/klucza";
        let data = [0u8; 16];

        let res = storage.put(key, &data).await;
        assert!(res.is_ok());

        let res = storage.get(key).await;
        assert_eq!(res, Some(Vec::from(data)));
    }

    #[tokio::test]
    #[timeout(500)]
    async fn key_forbidden_character_2() {
        let root_storage_dir = tempdir().unwrap();
        let mut storage = build_stable_storage(root_storage_dir.path().to_path_buf()).await;

        let key1 = "to_jest\0_wredna_nazwa_klucza";
        let key2 = "to_jest";
        let data = [0u8; 16];

        let res = storage.put(key1, &data).await;
        assert!(res.is_ok());

        let res = storage.get(key1).await;
        assert_eq!(res, Some(Vec::from(data)));

        let res = storage.get(key2).await;
        assert_eq!(res, None);
    }
}
