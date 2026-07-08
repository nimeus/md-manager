//! Integration tests for the built-in OAuth 2.1 authorization server (migration 0009).
//!
//! Requires the local dev Postgres with roles `md_owner`/`md_app` (see CLAUDE.md).
//! Run: `cargo test -p mdm-db --test oauth`

use mdm_core::model::{ActorType, AuthContext, OrgRole};
use mdm_db::Db;
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

// RFC 7636 Appendix B PKCE pair.
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const RESOURCE: &str = "https://api.test/mcp";
const REDIRECT: &str = "https://claude.ai/api/mcp/auth_callback";

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

/// Bootstrap a tenant and return `(user_id, org_id)` for an owner of a fresh org.
async fn tenant(db: &Db, email: &str, slug: &str) -> (Uuid, Uuid) {
    let (org, user, _key) = db
        .bootstrap(email, "Tester", slug, "Org", "cli")
        .await
        .expect("bootstrap");
    (user.id, org.id)
}

/// Register a public client and run authorize → approve, returning `(client_db_id, code)`.
async fn flow_to_code(db: &Db, user_id: Uuid, org_id: Uuid) -> (Uuid, String) {
    let client = db
        .register_oauth_client("Claude", &[REDIRECT.to_string()], true)
        .await
        .expect("register client");
    assert!(
        client.client_secret.is_none(),
        "public client has no secret"
    );
    let info = db.find_oauth_client(&client.client_id).await.expect("find");

    let req = db
        .create_authorization_request(
            info.db_id,
            REDIRECT,
            CHALLENGE,
            "S256",
            RESOURCE,
            "mcp",
            Some("state-123"),
            600,
        )
        .await
        .expect("create request");

    let display = db
        .get_authorization_request_display(req)
        .await
        .expect("display");
    assert_eq!(display.client_name, "Claude");
    assert_eq!(display.scope, "mcp");

    let minted = db
        .approve_authorization_request(req, user_id, Some(org_id), false, 60)
        .await
        .expect("approve");
    assert_eq!(minted.redirect_uri, REDIRECT);
    assert_eq!(minted.state.as_deref(), Some("state-123"));
    (info.db_id, minted.code)
}

#[tokio::test]
#[serial]
async fn full_authorization_code_flow() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;

    let tokens = db
        .exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .expect("exchange");
    assert!(tokens.access_token.starts_with("mo_"));
    assert!(tokens.refresh_token.starts_with("mor_"));
    assert_eq!(tokens.access_expires_in, 3600);

    // The access token resolves to the bound (user, org) with the membership role.
    let mdm_db::OAuthAccess::Org(ctx) = db
        .authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
        .await
        .expect("validate access token")
    else {
        panic!("expected a single-org token");
    };
    assert_eq!(ctx.user_id, user_id);
    assert_eq!(ctx.org_id, org_id);
    assert_eq!(ctx.actor_type, ActorType::Agent);
    assert_eq!(ctx.org_role, OrgRole::Owner);

    // Wrong audience is rejected (RFC 8707).
    assert!(
        db.authenticate_oauth_access_token(&tokens.access_token, "https://api.test/WRONG")
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn code_is_single_use() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;

    assert!(
        db.exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000
        )
        .await
        .is_ok()
    );
    // Replaying the same code fails.
    assert!(
        db.exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[serial]
async fn pkce_and_redirect_and_audience_are_enforced() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;

    // Wrong PKCE verifier.
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;
    assert!(
        db.exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            "wrong-verifier-wrong-verifier-wrong-verifier",
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .is_err()
    );

    // Wrong redirect_uri.
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;
    assert!(
        db.exchange_auth_code(
            cid,
            &code,
            "https://evil.example/cb",
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000
        )
        .await
        .is_err()
    );

    // Wrong audience at the token endpoint.
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;
    assert!(
        db.exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some("https://api.test/WRONG"),
            3600,
            2_592_000
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[serial]
async fn request_is_single_use() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let client = db
        .register_oauth_client("Claude", &[REDIRECT.to_string()], true)
        .await
        .unwrap();
    let info = db.find_oauth_client(&client.client_id).await.unwrap();
    let req = db
        .create_authorization_request(
            info.db_id, REDIRECT, CHALLENGE, "S256", RESOURCE, "mcp", None, 600,
        )
        .await
        .unwrap();
    assert!(
        db.approve_authorization_request(req, user_id, Some(org_id), false, 60)
            .await
            .is_ok()
    );
    // Second approval of the same request fails (consumed).
    assert!(
        db.approve_authorization_request(req, user_id, Some(org_id), false, 60)
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn refresh_rotates_and_reuse_kills_family() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;
    let t1 = db
        .exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .unwrap();

    // Rotate: refresh1 → (access2, refresh2).
    let t2 = db
        .refresh_oauth_token(cid, &t1.refresh_token, 3600, 2_592_000)
        .await
        .expect("rotate");
    assert_ne!(t1.refresh_token, t2.refresh_token);
    assert!(
        db.authenticate_oauth_access_token(&t2.access_token, RESOURCE)
            .await
            .is_ok()
    );

    // Reusing the OLD refresh is a theft signal → invalid_grant + family revoked.
    assert!(
        db.refresh_oauth_token(cid, &t1.refresh_token, 3600, 2_592_000)
            .await
            .is_err()
    );
    // The whole family is now dead: refresh2 and access2 no longer work.
    assert!(
        db.refresh_oauth_token(cid, &t2.refresh_token, 3600, 2_592_000)
            .await
            .is_err()
    );
    assert!(
        db.authenticate_oauth_access_token(&t2.access_token, RESOURCE)
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn revoke_access_token() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let (cid, code) = flow_to_code(&db, user_id, org_id).await;
    let tokens = db
        .exchange_auth_code(
            cid,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .unwrap();

    assert!(
        db.authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
            .await
            .is_ok()
    );
    db.revoke_oauth_token(&tokens.access_token).await.unwrap();
    assert!(
        db.authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn code_bound_to_its_client() {
    let db = setup().await;
    let (user_id, org_id) = tenant(&db, "a@x.com", "acme").await;
    let (_cid, code) = flow_to_code(&db, user_id, org_id).await;

    // A DIFFERENT client cannot redeem the code.
    let other = db
        .register_oauth_client("Other", &[REDIRECT.to_string()], true)
        .await
        .unwrap();
    let other_info = db.find_oauth_client(&other.client_id).await.unwrap();
    assert!(
        db.exchange_auth_code(
            other_info.db_id,
            &code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000
        )
        .await
        .is_err()
    );
}

/// Run a full connect (register → approve → token), returning `(client_id, tokens)`.
async fn connect(db: &Db, user_id: Uuid, org_id: Uuid) -> (String, mdm_db::IssuedTokens) {
    let client = db
        .register_oauth_client("Claude", &[REDIRECT.to_string()], true)
        .await
        .unwrap();
    let info = db.find_oauth_client(&client.client_id).await.unwrap();
    let req = db
        .create_authorization_request(
            info.db_id, REDIRECT, CHALLENGE, "S256", RESOURCE, "mcp", None, 600,
        )
        .await
        .unwrap();
    let minted = db
        .approve_authorization_request(req, user_id, Some(org_id), false, 60)
        .await
        .unwrap();
    let tokens = db
        .exchange_auth_code(
            info.db_id,
            &minted.code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .unwrap();
    (client.client_id, tokens)
}

fn ctx(user_id: Uuid, org_id: Uuid) -> AuthContext {
    AuthContext {
        org_id,
        user_id,
        actor_type: ActorType::User,
        org_role: OrgRole::Owner,
    }
}

#[tokio::test]
#[serial]
async fn grant_list_switch_revoke() {
    let db = setup().await;
    let (user, org_a) = tenant(&db, "a@x.com", "acme").await;
    // Same user owns a second org (bootstrap reuses the user by email).
    let org_b = db
        .bootstrap("a@x.com", "Tester", "globex", "Org B", "k2")
        .await
        .unwrap()
        .0
        .id;
    let c = ctx(user, org_a);

    let (client_id, tokens) = connect(&db, user, org_a).await;

    // Listed once, bound to org A.
    let grants = db.list_oauth_grants(&c).await.unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].org_id, org_a);
    assert_eq!(grants[0].client_id, client_id);

    // Switch org A → org B: the SAME access token now resolves to org B (no reconnect).
    db.switch_oauth_grant(&c, &client_id, org_a, org_b)
        .await
        .unwrap();
    let grants = db.list_oauth_grants(&c).await.unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].org_id, org_b, "grant moved to org B");
    let mdm_db::OAuthAccess::Org(actx) = db
        .authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
        .await
        .unwrap()
    else {
        panic!("expected a single-org token");
    };
    assert_eq!(actx.org_id, org_b, "live token now operates in org B");

    // Revoke in org B: grant gone + token dead.
    db.revoke_oauth_grant(&c, &client_id, org_b).await.unwrap();
    assert!(db.list_oauth_grants(&c).await.unwrap().is_empty());
    assert!(
        db.authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn switch_to_non_member_org_is_forbidden() {
    let db = setup().await;
    let (user, org_a) = tenant(&db, "a@x.com", "acme").await;
    // A different user's org — `user` is not a member.
    let org_c = db
        .bootstrap("c@x.com", "C", "ccorp", "C Org", "k")
        .await
        .unwrap()
        .0
        .id;
    let c = ctx(user, org_a);
    let (client_id, _t) = connect(&db, user, org_a).await;

    assert!(
        db.switch_oauth_grant(&c, &client_id, org_a, org_c)
            .await
            .is_err()
    );
    // Unchanged — still in org A.
    let grants = db.list_oauth_grants(&c).await.unwrap();
    assert_eq!(grants[0].org_id, org_a);
}

#[tokio::test]
#[serial]
async fn all_orgs_token_resolves_per_call() {
    let db = setup().await;
    let (user, org_a) = tenant(&db, "a@x.com", "acme").await;
    let org_b = db
        .bootstrap("a@x.com", "Tester", "globex", "Org B", "k2")
        .await
        .unwrap()
        .0
        .id;

    // Connect with "all my organizations" (org_id None, all_orgs true).
    let client = db
        .register_oauth_client("Claude", &[REDIRECT.to_string()], true)
        .await
        .unwrap();
    let info = db.find_oauth_client(&client.client_id).await.unwrap();
    let req = db
        .create_authorization_request(
            info.db_id, REDIRECT, CHALLENGE, "S256", RESOURCE, "mcp", None, 600,
        )
        .await
        .unwrap();
    let minted = db
        .approve_authorization_request(req, user, None, true, 60)
        .await
        .unwrap();
    let tokens = db
        .exchange_auth_code(
            info.db_id,
            &minted.code,
            REDIRECT,
            VERIFIER,
            Some(RESOURCE),
            3600,
            2_592_000,
        )
        .await
        .unwrap();

    // The access token resolves to AllOrgs (not a single org).
    let user_id = match db
        .authenticate_oauth_access_token(&tokens.access_token, RESOURCE)
        .await
        .unwrap()
    {
        mdm_db::OAuthAccess::AllOrgs { user_id } => user_id,
        mdm_db::OAuthAccess::Org(_) => panic!("expected all-orgs"),
    };
    assert_eq!(user_id, user);

    // Per-call org resolution: both of the user's orgs work (by slug and by id); others don't.
    assert_eq!(
        db.authenticate_oauth_user_in_org(user, "acme")
            .await
            .unwrap()
            .org_id,
        org_a
    );
    assert_eq!(
        db.authenticate_oauth_user_in_org(user, "globex")
            .await
            .unwrap()
            .org_id,
        org_b
    );
    assert!(
        db.authenticate_oauth_user_in_org(user, &org_b.to_string())
            .await
            .is_ok()
    );
    assert!(
        db.authenticate_oauth_user_in_org(user, "nonexistent")
            .await
            .is_err()
    );
}
