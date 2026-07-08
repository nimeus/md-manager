//! Integration tests for member management + invite-link acceptance.
//! Run: `cargo test -p mdm-db --test members`

use mdm_core::model::OrgRole;
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
    Db::connect(
        &app_url(),
        5,
        "test-pepper".into(),
        1_000_000,
        30,
        1_000_000,
    )
    .await
    .expect("connect app")
}

#[tokio::test]
#[serial]
async fn member_roles_and_removal() {
    let db = setup().await;
    let (org, alice, key) = db
        .bootstrap("a@x.com", "Alice", "acme", "Acme", "k")
        .await
        .unwrap();
    let ctx = db.authenticate_api_key(&key.secret).await.unwrap(); // owner

    // Invite Bob (member); he joins by signing in with the matching Google email.
    db.create_invitation(&ctx, "bob@x.com", OrgRole::Member)
        .await
        .unwrap();
    let bob = db
        .provision_google_user("g_bob", "bob@x.com", "Bob")
        .await
        .unwrap();

    let members = db.list_members(&ctx).await.unwrap();
    assert_eq!(members.len(), 2);
    assert!(
        members
            .iter()
            .any(|m| m.user_id == bob.user_id && m.role == OrgRole::Member)
    );

    // Promote Bob to admin.
    db.update_member_role(&ctx, bob.user_id, OrgRole::Admin)
        .await
        .unwrap();
    assert!(
        db.list_members(&ctx)
            .await
            .unwrap()
            .iter()
            .any(|m| m.user_id == bob.user_id && m.role == OrgRole::Admin)
    );

    // Can't remove or demote the last owner (Alice).
    assert!(db.remove_member(&ctx, alice.id).await.is_err());
    assert!(
        db.update_member_role(&ctx, alice.id, OrgRole::Member)
            .await
            .is_err()
    );

    // Remove Bob.
    db.remove_member(&ctx, bob.user_id).await.unwrap();
    assert_eq!(db.list_members(&ctx).await.unwrap().len(), 1);
    let _ = org;
}

#[tokio::test]
#[serial]
async fn accept_invitation_by_link_is_single_use() {
    let db = setup().await;
    let (org, _alice, key) = db
        .bootstrap("a@x.com", "Alice", "acme", "Acme", "k")
        .await
        .unwrap();
    let ctx = db.authenticate_api_key(&key.secret).await.unwrap();

    // Invite carol@x.com, but Carol signs in with a DIFFERENT email (no auto-accept),
    // then joins via the link token.
    let inv = db
        .create_invitation(&ctx, "carol@x.com", OrgRole::Viewer)
        .await
        .unwrap();
    let carol = db
        .provision_google_user("g_carol", "carol-other@x.com", "Carol")
        .await
        .unwrap();

    let joined = db
        .accept_invitation_by_token(carol.user_id, &inv.token)
        .await
        .unwrap();
    assert_eq!(joined.id, org.id);
    assert!(
        db.list_user_orgs(carol.user_id)
            .await
            .unwrap()
            .iter()
            .any(|o| o.id == org.id && o.role == OrgRole::Viewer)
    );

    // Single-use: the same link can't be redeemed again.
    assert!(
        db.accept_invitation_by_token(carol.user_id, &inv.token)
            .await
            .is_err()
    );
    // Garbage token → not found.
    assert!(
        db.accept_invitation_by_token(carol.user_id, "inv_deadbeef")
            .await
            .is_err()
    );
}
