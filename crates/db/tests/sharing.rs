//! Integration tests for public/private share links.
//! Run: `cargo test -p mdm-db --test sharing`

use mdm_core::Error;
use mdm_db::Db;
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;

fn owner_url() -> String {
    std::env::var("MDM_TEST_OWNER_URL").unwrap_or_else(|_| {
        "postgres://md_owner:md_owner_dev@localhost:5432/md_manager_test".into()
    })
}
fn app_url() -> String {
    std::env::var("MDM_TEST_APP_URL")
        .unwrap_or_else(|_| "postgres://md_app:md_app_dev@localhost:5432/md_manager_test".into())
}

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
    Db::connect(&app_url(), 5, "test-pepper".into(), 1_000_000, 30, 1_000_000)
        .await
        .expect("connect app")
}

#[tokio::test]
#[serial]
async fn share_audiences_are_enforced() {
    let db = setup().await;
    let (_org, alice, key) = db
        .bootstrap("a@x.com", "Alice", "acme", "Acme", "k")
        .await
        .unwrap();
    let ctx = db.authenticate_api_key(&key.secret).await.unwrap();
    let proj = db.create_project(&ctx, "docs", "Docs").await.unwrap();
    let doc = db
        .create_document(&ctx, proj.id, "guides/x", "X", "# X\nhello\n")
        .await
        .unwrap();

    // Outsiders: a member of another org (bob) and a non-member (carol).
    let bob = db.provision_google_user("g_bob", "bob@x.com", "Bob").await.unwrap();
    let carol = db.provision_google_user("g_carol", "carol@x.com", "Carol").await.unwrap();

    // public — anyone, even anonymous.
    let pub_t = db.create_share_link(&ctx, doc.id, "public", &[], None).await.unwrap().token;
    assert!(db.resolve_share_link(&pub_t, None).await.is_ok());
    assert!(db.resolve_share_link(&pub_t, Some(carol.user_id)).await.is_ok());

    // members — only signed-in members of the doc's org.
    let mem_t = db.create_share_link(&ctx, doc.id, "members", &[], None).await.unwrap().token;
    assert!(matches!(db.resolve_share_link(&mem_t, None).await, Err(Error::Unauthorized)));
    assert!(db.resolve_share_link(&mem_t, Some(alice.id)).await.is_ok());
    assert!(matches!(
        db.resolve_share_link(&mem_t, Some(carol.user_id)).await,
        Err(Error::Forbidden)
    ));

    // emails — only allow-listed recipients (after sign-in).
    let em_t = db
        .create_share_link(&ctx, doc.id, "emails", &["bob@x.com".into()], None)
        .await
        .unwrap()
        .token;
    assert!(matches!(db.resolve_share_link(&em_t, None).await, Err(Error::Unauthorized)));
    assert!(db.resolve_share_link(&em_t, Some(bob.user_id)).await.is_ok());
    assert!(matches!(
        db.resolve_share_link(&em_t, Some(carol.user_id)).await,
        Err(Error::Forbidden)
    ));

    // emails audience requires at least one recipient.
    assert!(db.create_share_link(&ctx, doc.id, "emails", &[], None).await.is_err());
    // unknown audience rejected.
    assert!(db.create_share_link(&ctx, doc.id, "nope", &[], None).await.is_err());
}
