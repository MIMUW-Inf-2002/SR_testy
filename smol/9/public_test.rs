#[cfg(test)]
mod tests {
    use ntest::timeout;
    use tokio::sync::oneshot::channel;

    use crate::solution::{
        DistributedStore, Node, Product, ProductType, Transaction, TransactionMessage,
        TwoPhaseResult,
    };
    use module_system::System;
    use uuid::Uuid;

    #[tokio::test]
    #[timeout(300)]
    async fn transaction_with_two_nodes_completes() {
        // Given:
        let mut system = System::new().await;
        let (transaction_done_tx, transaction_done_rx) = channel();
        let products = vec![Product {
            identifier: Uuid::new_v4(),
            pr_type: ProductType::Electronics,
            price: 180,
        }];
        let node0 = system.register_module(Node::new(products.clone())).await;
        let node1 = system.register_module(Node::new(products)).await;
        let distributed_store = system
            .register_module(DistributedStore::new(vec![node0, node1]))
            .await;

        // When:
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        // Then:
        assert_eq!(TwoPhaseResult::Ok, transaction_done_rx.await.unwrap());
        system.shutdown().await;
    }

    #[tokio::test]
    #[timeout(300)]
    async fn prices_cant_be_negative() {
        let mut system = System::new().await;
        let products = vec![
            Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Electronics,
                price: 200,
            },
            Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Electronics,
                price: 100,
            },
        ];

        let node0 = system.register_module(Node::new(products.clone())).await;
        let node1 = system.register_module(Node::new(products)).await;
        let distributed_store = system
            .register_module(DistributedStore::new(vec![node0, node1]))
            .await;

        let (transaction_done_tx, transaction_done_rx) = channel();
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        assert_eq!(TwoPhaseResult::Ok, transaction_done_rx.await.unwrap());

        let (transaction_done_tx, transaction_done_rx) = channel();
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        assert_eq!(TwoPhaseResult::Abort, transaction_done_rx.await.unwrap());
    }

    #[tokio::test]
    #[timeout(300)]
    async fn filtering_works() {
        let mut system = System::new().await;
        let products = vec![
            Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Electronics,
                price: 100,
            },
            Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Toys,
                price: 100,
            },
            Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Books,
                price: 100,
            },
        ];

        let node0 = system.register_module(Node::new(products.clone())).await;
        let node1 = system.register_module(Node::new(products.clone())).await;
        let node2 = system.register_module(Node::new(products)).await;
        let distributed_store = system
            .register_module(DistributedStore::new(vec![node0, node1, node2]))
            .await;

        let (transaction_done_tx, transaction_done_rx) = channel();
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        assert_eq!(TwoPhaseResult::Ok, transaction_done_rx.await.unwrap());

        let (transaction_done_tx, transaction_done_rx) = channel();
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Toys,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        assert_eq!(TwoPhaseResult::Ok, transaction_done_rx.await.unwrap());

        let (transaction_done_tx, transaction_done_rx) = channel();
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Books,
                    shift: -50,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            })
            .await;

        assert_eq!(TwoPhaseResult::Ok, transaction_done_rx.await.unwrap());
    }

    #[tokio::test]
    #[timeout(469)]
    async fn many_nodes() {
        let mut system = System::new().await;
        let mut products = vec![];
        for i in 1..101 {
            products.push(Product {
                identifier: Uuid::new_v4(),
                pr_type: ProductType::Electronics,
                price: (i + 1 + 100) as u64,
            });
        }

        let mut nodes = vec![];
        for _ in 0..100 {
            nodes.push(system.register_module(Node::new(products.clone())).await);
        }

        let distributed_store = system.register_module(DistributedStore::new(nodes)).await;

        for i in 0..100 {
            let (transaction_done_tx, transaction_done_rx) = channel();
            let msg = TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -1,
                },
                completed_callback: Box::new(|result| {
                    Box::pin(async move {
                        transaction_done_tx.send(result).unwrap();
                    })
                }),
            };

            distributed_store.send(msg).await;

            assert_eq!(
                (TwoPhaseResult::Ok, i),
                (transaction_done_rx.await.unwrap(), i)
            );
        }
    }
}
