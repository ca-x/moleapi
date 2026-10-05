//! Imported by the existing database integration target so all remote engines run in Actions.
use moleapi_core::{DataConfig, DataSource, NetworkPolicy, Protocol, RequestSpec};
use moleapi_data::{Cell, Connection};
use serde_json::json;
use std::time::Duration;
pub async fn verify_configured_data_clients() {
    for (key, source) in [
        ("MOLEAPI_TEST_POSTGRES_URL", DataSource::Postgresql),
        ("MOLEAPI_TEST_MYSQL_URL", DataSource::Mysql),
    ] {
        let Ok(url) = std::env::var(key) else {
            continue;
        };
        let config = DataConfig {
            source,
            tls: false,
            ..Default::default()
        };
        let request:RequestSpec=serde_json::from_value(json!({"id":"data-fixture","name":"Data fixture","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":5000,"verify_tls":true,"follow_redirects":false,"assertions":[],"examples":[]})).unwrap();
        let mut connection = Connection::open(
            &request,
            &config,
            NetworkPolicy {
                allow_private_network: true,
            },
            &std::env::current_exe().unwrap(),
        )
        .await
        .unwrap();
        let name = format!("moleapi_data_{}", uuid::Uuid::new_v4().simple());
        let table = moleapi_data::quoted(&name, source);
        let timeout = Duration::from_secs(5);
        let blob = if source == DataSource::Postgresql {
            "BYTEA"
        } else {
            "VARBINARY(16)"
        };
        connection.query(&format!("CREATE TABLE {table} (id BIGINT, amount DECIMAL(38,20), note TEXT, payload {blob})"),false,timeout).await.unwrap();
        let bytes = if source == DataSource::Postgresql {
            "decode('00ff41','hex')"
        } else {
            "X'00ff41'"
        };
        connection.query(&format!("INSERT INTO {table} (id,amount,note,payload) VALUES (9007199254740993,1.23000000000000000001,'fixture',{bytes})"),false,timeout).await.unwrap();
        let result = connection
            .query(
                &format!("SELECT id AS repeated,amount AS repeated,note,payload FROM {table}"),
                true,
                timeout,
            )
            .await
            .unwrap();
        assert_eq!(result.columns[0].name, result.columns[1].name);
        assert_eq!(
            result.rows[0][0],
            Cell::Integer {
                value: "9007199254740993".into()
            }
        );
        assert_eq!(
            result.rows[0][1],
            Cell::Decimal {
                value: "1.23000000000000000001".into()
            }
        );
        assert_eq!(
            result.rows[0][2],
            Cell::Text {
                value: "fixture".into()
            }
        );
        assert_eq!(
            result.rows[0][3],
            Cell::Binary {
                base64: "AP9B".into(),
                bytes: 3
            }
        );
        assert!(
            connection
                .query(
                    &format!("UPDATE {table} SET note='must-not-write'"),
                    true,
                    timeout
                )
                .await
                .is_err()
        );
        if source == DataSource::Postgresql {
            assert!(
                connection
                    .query(
                        "SELECT pg_catalog.set_config('bytea_output','escape',false)",
                        true,
                        timeout
                    )
                    .await
                    .is_err()
            );
            let result = connection
                .query("SELECT decode('00ff41','hex')", true, timeout)
                .await
                .unwrap();
            assert_eq!(
                result.rows[0][0],
                Cell::Binary {
                    base64: "AP9B".into(),
                    bytes: 3
                }
            );
            assert!(connection.query(&format!("WITH changed AS (UPDATE {table} SET note='must-not-write' RETURNING id) SELECT id FROM changed"),true,timeout).await.is_err());
        }
        let result = connection
            .query(&format!("SELECT note FROM {table}"), true, timeout)
            .await
            .unwrap();
        assert_eq!(
            result.rows[0][0],
            Cell::Text {
                value: "fixture".into()
            }
        );
        let (schema, _) = connection.schema().await.unwrap();
        assert!(
            schema
                .iter()
                .any(|item| item.name == name && item.columns.len() == 4)
        );
        connection
            .query(&format!("DROP TABLE {table}"), false, timeout)
            .await
            .unwrap();
        // Capping preserves returned rows and disposes the MySQL stream instead
        // of draining a potentially endless result while returning an error.
        {
            let mut capped = Connection::open(
                &request,
                &config,
                NetworkPolicy {
                    allow_private_network: true,
                },
                &std::env::current_exe().unwrap(),
            )
            .await
            .unwrap();
            let id_sql = if source == DataSource::Postgresql {
                "SELECT pg_backend_pid()"
            } else {
                "SELECT CONNECTION_ID()"
            };
            let result = capped.query(id_sql, true, timeout).await.unwrap();
            let Cell::Integer { value: capped_id } = &result.rows[0][0] else {
                panic!("missing database connection ID")
            };
            let capped_id = capped_id.clone();
            let digits = "(SELECT 0 AS n UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9)";
            let sql = if source == DataSource::Postgresql {
                "SELECT n FROM generate_series(1,10000) n".into()
            } else {
                format!(
                    "SELECT a.n FROM {digits} a CROSS JOIN {digits} b CROSS JOIN {digits} c CROSS JOIN {digits} d"
                )
            };
            let result = tokio::time::timeout(timeout, capped.query(&sql, true, timeout))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result.rows.len(), moleapi_data::MAX_ROWS);
            assert!(result.truncated);
            assert!(capped.is_closed());
            let count_sql = if source == DataSource::Postgresql {
                format!("SELECT COUNT(*) FROM pg_stat_activity WHERE pid={capped_id}")
            } else {
                format!("SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID={capped_id}")
            };
            tokio::time::timeout(timeout, async {
                loop {
                    let rows = connection
                        .query(&count_sql, true, timeout)
                        .await
                        .unwrap()
                        .rows;
                    if matches!(&rows[0][0], Cell::Integer { value } if value == "0") {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("capped database socket survived disposal");
        }
        // Actual SDK cancellation plus owner disposal must release sleeping
        // queries. Inspect server-owned session IDs rather than inferring cleanup.
        let mut sleeping = Connection::open(
            &request,
            &config,
            NetworkPolicy {
                allow_private_network: true,
            },
            &std::env::current_exe().unwrap(),
        )
        .await
        .unwrap();
        let (id_sql, sleep_sql) = if source == DataSource::Postgresql {
            ("SELECT pg_backend_pid()", "SELECT pg_sleep(30)")
        } else {
            ("SELECT CONNECTION_ID()", "SELECT SLEEP(30)")
        };
        let rows = sleeping.query(id_sql, true, timeout).await.unwrap().rows;
        let Cell::Integer { value: sleeping_id } = &rows[0][0] else {
            panic!("missing sleeping connection ID")
        };
        let sleeping_id = sleeping_id.clone();
        let cancel = sleeping.cancellation();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(100),
                sleeping.query(sleep_sql, true, timeout)
            )
            .await
            .is_err()
        );
        tokio::time::timeout(timeout, cancel.request())
            .await
            .unwrap()
            .unwrap();
        drop(sleeping);
        let count_sql = if source == DataSource::Postgresql {
            format!("SELECT COUNT(*) FROM pg_stat_activity WHERE pid={sleeping_id}")
        } else {
            format!("SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE ID={sleeping_id}")
        };
        tokio::time::timeout(timeout, async {
            loop {
                let rows = connection
                    .query(&count_sql, true, timeout)
                    .await
                    .unwrap()
                    .rows;
                if matches!(&rows[0][0], Cell::Integer { value } if value == "0") {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("canceled database socket survived owner disposal");
        let mut denied = request.clone();
        denied.protocol = Protocol::Data {
            config: Box::new(config.clone()),
        };
        assert!(
            Connection::open(
                &denied,
                &config,
                NetworkPolicy {
                    allow_private_network: false
                },
                &std::env::current_exe().unwrap()
            )
            .await
            .is_err()
        );
    }
}
