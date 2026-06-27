//! Integration tests against a real Postgres (`md_manager_test`).
//!
//! Requires the local dev Postgres with roles `md_owner`/`md_app` (see README/CLAUDE.md).
//! Override URLs via `MDM_TEST_OWNER_URL` / `MDM_TEST_APP_URL` if needed.
//!
//! Run: `cargo test -p mdm-db`

use mdm_core::model::{ActorType, OrgRole, VersionKind};
use mdm_db::{Db, UpdateOutcome};
use sqlx::postgres::PgPoolOptions;

fn owner_url() -> String {
    std::env::var("MDM_TEST_OWNER_URL")
        .unwrap_or_else(|_| "postgres://md_owner:md_owner_dev@localhost:5432/md_manager_test".into())
}
fn app_url() -> String {
    std::env::var("MDM_TEST_APP_URL")
        .unwrap_or_else(|_| "postgres://md_app:md_app_dev@localhost:5432/md_manager_test".into())
}

/// Reset the schema and run migrations, returning a connected app-role `Db`.
async fn setup() -> Db {
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&owner_url())
        .await
        .expect("connect as owner");
    for stmt in [
        "DROP SCHEMA public CASCADE",
        "CREATE SCHEMA public AUTHORIZATION md_owner",
        "GRANT USAGE ON SCHEMA public TO md_app",
    ] {
        sqlx::query(stmt).execute(&admin).await.expect(stmt);
    }
    admin.close().await;

    Db::run_migrations(&owner_url()).await.expect("migrate");
    Db::connect(&app_url(), 5, "test-pepper".into(), 1_000_000, 30)
        .await
        .expect("connect app")
}

#[tokio::test]
async fn full_db_layer() {
    let db = setup().await;
    db.assert_app_role_not_bypassrls()
        .await
        .expect("app role must not bypass RLS");

    // --- bootstrap two tenants ------------------------------------------------
    let (org_a, _user_a, key_a) = db
        .bootstrap("a@example.com", "Alice", "acme", "Acme", "alice-cli")
        .await
        .expect("bootstrap A");
    let (_org_b, _user_b, key_b) = db
        .bootstrap("b@example.com", "Bob", "globex", "Globex", "bob-cli")
        .await
        .expect("bootstrap B");

    let ctx_a = db.authenticate_api_key(&key_a.secret).await.expect("auth A");
    let ctx_b = db.authenticate_api_key(&key_b.secret).await.expect("auth B");
    assert_eq!(ctx_a.org_id, org_a.id);
    assert_eq!(ctx_a.actor_type, ActorType::Agent);
    assert_eq!(ctx_a.org_role, OrgRole::Admin);

    // bad key is rejected
    assert!(db.authenticate_api_key("mk_deadbeef").await.is_err());

    // --- create project + document in org A ----------------------------------
    let proj = db.create_project(&ctx_a, "docs", "Docs").await.expect("project");
    let doc = db
        .create_document(&ctx_a, proj.id, "guides/setup", "Setup", "# Setup\nHello world\n")
        .await
        .expect("create doc");
    assert_eq!(doc.current_version, 1);

    // --- tenant isolation: org B sees nothing of org A -----------------------
    assert!(db.list_projects(&ctx_b).await.unwrap().is_empty());
    assert!(matches!(
        db.get_document(&ctx_b, doc.id).await,
        Err(mdm_core::Error::NotFound)
    ));
    assert!(db.search(&ctx_b, None, "Hello", 10).await.unwrap().is_empty());
    // org A does see it
    assert!(db.get_document(&ctx_a, doc.id).await.is_ok());

    // --- full-text search (org A) --------------------------------------------
    let hits = db.search(&ctx_a, None, "hello world", 10).await.expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document_id, doc.id);

    // --- optimistic concurrency ----------------------------------------------
    let upd = db
        .update_document(&ctx_a, doc.id, "# Setup\nv2\n", 1, VersionKind::Checkpoint)
        .await
        .expect("update");
    let v2 = match upd {
        UpdateOutcome::Updated(d) => d,
        UpdateOutcome::Conflict { .. } => panic!("unexpected conflict"),
    };
    assert_eq!(v2.current_version, 2);

    // stale write -> conflict carrying base + current
    let conflict = db
        .update_document(&ctx_a, doc.id, "# Setup\nv3\n", 1, VersionKind::Checkpoint)
        .await
        .expect("update call ok");
    match conflict {
        UpdateOutcome::Conflict { current_version, current_content, base_content } => {
            assert_eq!(current_version, 2);
            assert!(current_content.contains("v2"));
            assert!(base_content.contains("Hello world"));
        }
        UpdateOutcome::Updated(_) => panic!("expected a conflict"),
    }

    // --- atomic append --------------------------------------------------------
    let appended = db.append_to_document(&ctx_a, doc.id, "appended line\n").await.expect("append");
    assert_eq!(appended.current_version, 3);
    assert!(appended.content.contains("v2"));
    assert!(appended.content.contains("appended line"));

    // --- restore --------------------------------------------------------------
    let restored = db.restore_version(&ctx_a, doc.id, 1).await.expect("restore");
    assert_eq!(restored.current_version, 4);
    assert!(restored.content.contains("Hello world"));

    // --- history --------------------------------------------------------------
    let history = db.get_history(&ctx_a, doc.id).await.expect("history");
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].version, 4); // newest first

    // --- soft delete + undelete ----------------------------------------------
    db.delete_document(&ctx_a, doc.id).await.expect("delete");
    assert!(matches!(db.get_document(&ctx_a, doc.id).await, Err(mdm_core::Error::NotFound)));
    assert!(db.search(&ctx_a, None, "hello", 10).await.unwrap().is_empty());
    db.undelete_document(&ctx_a, doc.id).await.expect("undelete");
    assert!(db.get_document(&ctx_a, doc.id).await.is_ok());

    // --- RBAC: a viewer key cannot write -------------------------------------
    let viewer_key = db
        .create_api_key(&ctx_a, "viewer-bot", OrgRole::Viewer)
        .await
        .expect("mint viewer key");
    let ctx_viewer = db.authenticate_api_key(&viewer_key.secret).await.expect("auth viewer");
    assert_eq!(ctx_viewer.org_role, OrgRole::Viewer);
    assert!(db.get_document(&ctx_viewer, doc.id).await.is_ok()); // can read
    assert!(matches!(
        db.create_document(&ctx_viewer, proj.id, "x", "X", "x").await,
        Err(mdm_core::Error::Forbidden)
    ));

    // --- revoke kills the key -------------------------------------------------
    db.revoke_api_key(&ctx_a, viewer_key.info.id).await.expect("revoke");
    assert!(db.authenticate_api_key(&viewer_key.secret).await.is_err());
}
