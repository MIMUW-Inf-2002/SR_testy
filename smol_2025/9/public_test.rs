#[cfg(test)]
mod tests {
    use crate::relay_util::{BoxedModuleSender, Sendee, SenderTo};
    use crate::solution::{
        DistributedStore, Node, Product, ProductType, StoreMsg, Transaction, TransactionMessage,
        TwoPhaseResult,
    };
    use module_system::{ModuleRef, System};
    use ntest::timeout;
    use std::any::Any;
    use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
    use tokio::sync::oneshot::channel;
    use uuid::Uuid;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
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
        let node0 = system
            .register_module(|_| Node::new(products.clone()))
            .await;
        let node1 = system.register_module(|_| Node::new(products)).await;
        let distributed_store = system
            .register_module(|sr| {
                DistributedStore::new(vec![Box::new(node0), Box::new(node1)], Box::new(sr))
            })
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

    #[derive(Clone)]
    pub struct NodeSpy {
        original: ModuleRef<Node>,
        spy_tx: UnboundedSender<Box<dyn Any + Send>>,
    }

    #[async_trait::async_trait]
    impl SenderTo<Node> for NodeSpy {
        async fn send_message(&self, msg: Box<dyn Sendee<Node>>) {
            if let Some(msg) = (&*msg as &dyn Any).downcast_ref::<StoreMsg>() {
                self.spy_tx.send(Box::new(msg.clone())).unwrap();
            }
            self.original.send_message(msg).await;
        }

        fn cloned_box(&self) -> BoxedModuleSender<Node> {
            Box::new(self.clone())
        }
    }

    #[tokio::test]
    #[timeout(300)]
    async fn system_compiles_with_spies() {
        // Given:
        let mut system = System::new().await;
        let (spy_tx, mut spy_rx) = unbounded_channel();
        let node = system.register_module(|_| Node::new(vec![])).await;
        let node_spy = NodeSpy {
            original: node,
            spy_tx,
        };
        let distributed_store = system
            .register_module(|sr| DistributedStore::new(vec![Box::new(node_spy)], Box::new(sr)))
            .await;

        // When:
        distributed_store
            .send(TransactionMessage {
                transaction: Transaction {
                    pr_type: ProductType::Electronics,
                    shift: -50,
                },
                completed_callback: Box::new(|_result| Box::pin(async move {})),
            })
            .await;

        // Then:
        assert!(spy_rx.recv().await.unwrap().is::<StoreMsg>());
        system.shutdown().await;
    }

    // Tests below by Rentib, adapted by Kwasow for the 2025 edition

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

        let node0 = system
            .register_module(|_| Node::new(products.clone()))
            .await;
        let node1 = system.register_module(|_| Node::new(products)).await;
        let distributed_store = system
            .register_module(|sr| {
                DistributedStore::new(vec![Box::new(node0), Box::new(node1)], Box::new(sr))
            })
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

        let node0 = system
            .register_module(|_| Node::new(products.clone()))
            .await;
        let node1 = system
            .register_module(|_| Node::new(products.clone()))
            .await;
        let node2 = system.register_module(|_| Node::new(products)).await;
        let distributed_store = system
            .register_module(|sr| {
                DistributedStore::new(
                    vec![Box::new(node0), Box::new(node1), Box::new(node2)],
                    Box::new(sr),
                )
            })
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

        let mut nodes: Vec<BoxedModuleSender<Node>> = vec![];
        for _ in 0..100 {
            let boxed_ref = Box::new(
                system
                    .register_module(|_| Node::new(products.clone()))
                    .await,
            );

            nodes.push(boxed_ref);
        }

        let distributed_store = system
            .register_module(|sr| DistributedStore::new(nodes, Box::new(sr)))
            .await;

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

