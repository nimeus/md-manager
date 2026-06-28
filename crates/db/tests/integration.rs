//! Integration tests against a real Postgres (`md_manager_test`).
//!
//! Requires the local dev Postgres with roles `md_owner`/`md_app` (see README/CLAUDE.md).
//! Override URLs via `MDM_TEST_OWNER_URL` / `MDM_TEST_APP_URL` if needed.
//!
//! Run: `cargo test -p mdm-db`

use mdm_core::model::{ActorType, OrgRole, Role, VersionKind};
use mdm_db::{Db, EmbeddingStore, UpdateOutcome};
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

/// A `Db` with a tiny per-project document quota, for the quota test.
async fn connect_with_quota(max_docs_per_project: i64) -> Db {
    Db::connect(
        &app_url(),
        2,
        "test-pepper".into(),
        1_000_000,
        30,
        max_docs_per_project,
    )
    .await
    .expect("connect app (quota)")
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

    let ctx_a = db
        .authenticate_api_key(&key_a.secret)
        .await
        .expect("auth A");
    let ctx_b = db
        .authenticate_api_key(&key_b.secret)
        .await
        .expect("auth B");
    assert_eq!(ctx_a.org_id, org_a.id);
    assert_eq!(ctx_a.actor_type, ActorType::Agent);
    assert_eq!(ctx_a.org_role, OrgRole::Admin);

    // bad key is rejected
    assert!(db.authenticate_api_key("mk_deadbeef").await.is_err());

    // --- create project + document in org A ----------------------------------
    let proj = db
        .create_project(&ctx_a, "docs", "Docs")
        .await
        .expect("project");
    let doc = db
        .create_document(
            &ctx_a,
            proj.id,
            "guides/setup",
            "Setup",
            "# Setup\nHello world\n",
        )
        .await
        .expect("create doc");
    assert_eq!(doc.current_version, 1);

    // --- tenant isolation: org B sees nothing of org A -----------------------
    assert!(db.list_projects(&ctx_b).await.unwrap().is_empty());
    assert!(matches!(
        db.get_document(&ctx_b, doc.id).await,
        Err(mdm_core::Error::NotFound)
    ));
    assert!(
        db.search(&ctx_b, None, "Hello", 10)
            .await
            .unwrap()
            .is_empty()
    );
    // org A does see it
    assert!(db.get_document(&ctx_a, doc.id).await.is_ok());

    // --- full-text search (org A) --------------------------------------------
    let hits = db
        .search(&ctx_a, None, "hello world", 10)
        .await
        .expect("search");
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
        UpdateOutcome::Conflict {
            current_version,
            current_content,
            base_content,
        } => {
            assert_eq!(current_version, 2);
            assert!(current_content.contains("v2"));
            assert!(base_content.contains("Hello world"));
        }
        UpdateOutcome::Updated(_) => panic!("expected a conflict"),
    }

    // --- atomic append --------------------------------------------------------
    let appended = db
        .append_to_document(&ctx_a, doc.id, "appended line\n")
        .await
        .expect("append");
    assert_eq!(appended.current_version, 3);
    assert!(appended.content.contains("v2"));
    assert!(appended.content.contains("appended line"));

    // --- restore --------------------------------------------------------------
    let restored = db
        .restore_version(&ctx_a, doc.id, 1)
        .await
        .expect("restore");
    assert_eq!(restored.current_version, 4);
    assert!(restored.content.contains("Hello world"));

    // --- history --------------------------------------------------------------
    let history = db.get_history(&ctx_a, doc.id).await.expect("history");
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].version, 4); // newest first

    // --- soft delete + undelete ----------------------------------------------
    db.delete_document(&ctx_a, doc.id).await.expect("delete");
    assert!(matches!(
        db.get_document(&ctx_a, doc.id).await,
        Err(mdm_core::Error::NotFound)
    ));
    assert!(
        db.search(&ctx_a, None, "hello", 10)
            .await
            .unwrap()
            .is_empty()
    );
    db.undelete_document(&ctx_a, doc.id)
        .await
        .expect("undelete");
    assert!(db.get_document(&ctx_a, doc.id).await.is_ok());

    // --- RBAC: a viewer key cannot write -------------------------------------
    let viewer_key = db
        .create_api_key(&ctx_a, "viewer-bot", OrgRole::Viewer)
        .await
        .expect("mint viewer key");
    let ctx_viewer = db
        .authenticate_api_key(&viewer_key.secret)
        .await
        .expect("auth viewer");
    assert_eq!(ctx_viewer.org_role, OrgRole::Viewer);
    assert!(db.get_document(&ctx_viewer, doc.id).await.is_ok()); // can read
    assert!(matches!(
        db.create_document(&ctx_viewer, proj.id, "x", "X", "x")
            .await,
        Err(mdm_core::Error::Forbidden)
    ));

    // --- revoke kills the key -------------------------------------------------
    db.revoke_api_key(&ctx_a, viewer_key.info.id)
        .await
        .expect("revoke");
    assert!(db.authenticate_api_key(&viewer_key.secret).await.is_err());

    // --- categories (hierarchical, cross-project) ----------------------------
    let root = db
        .create_category(&ctx_a, None, "engineering", "Engineering")
        .await
        .expect("root cat");
    let child = db
        .create_category(&ctx_a, Some(root.id), "backend", "Backend")
        .await
        .expect("child cat");
    assert_eq!(child.parent_id, Some(root.id));
    // duplicate root slug rejected
    assert!(
        db.create_category(&ctx_a, None, "engineering", "Dup")
            .await
            .is_err()
    );

    db.categorize_document(&ctx_a, doc.id, child.id)
        .await
        .expect("categorize");
    let doc_cats = db.list_document_categories(&ctx_a, doc.id).await.unwrap();
    assert_eq!(doc_cats.len(), 1);
    assert_eq!(doc_cats[0].id, child.id);
    let in_cat = db
        .list_documents_in_category(&ctx_a, child.id)
        .await
        .unwrap();
    assert_eq!(in_cat.len(), 1);
    assert_eq!(in_cat[0].id, doc.id);

    // tenant isolation: org B can't categorize org A's doc, nor see A's categories
    assert!(db.list_categories(&ctx_b).await.unwrap().is_empty());
    assert!(matches!(
        db.categorize_document(&ctx_b, doc.id, child.id).await,
        Err(mdm_core::Error::NotFound)
    ));

    // --- RBAC lattice: per-doc deny + team grants + owner override -----------
    // A 'member' key shares the admin's user id but is clamped to member role, so it
    // exercises the deny path (which org owner/admin override).
    let member_key = db
        .create_api_key(&ctx_a, "member-bot", OrgRole::Member)
        .await
        .expect("member key");
    let ctx_member = db
        .authenticate_api_key(&member_key.secret)
        .await
        .expect("auth member");
    assert_eq!(ctx_member.org_role, OrgRole::Member);

    // baseline: a member can read + write
    assert!(db.get_document(&ctx_member, doc.id).await.is_ok());
    assert!(
        db.append_to_document(&ctx_member, doc.id, "by member\n")
            .await
            .is_ok()
    );

    // explicit per-doc deny locks the member out (read + write)
    db.grant_document(&ctx_a, doc.id, "user", ctx_member.user_id, Role::None)
        .await
        .expect("deny");
    assert!(matches!(
        db.get_document(&ctx_member, doc.id).await,
        Err(mdm_core::Error::Forbidden)
    ));
    assert!(matches!(
        db.update_document(&ctx_member, doc.id, "x", 99, VersionKind::Checkpoint)
            .await,
        Err(mdm_core::Error::Forbidden)
    ));
    // owner/admin override the deny
    assert!(db.get_document(&ctx_a, doc.id).await.is_ok());

    // a positive user grant does NOT beat an explicit deny on the same doc...
    db.grant_document(&ctx_a, doc.id, "user", ctx_member.user_id, Role::Editor)
        .await
        .expect("regrant editor");
    assert!(db.get_document(&ctx_member, doc.id).await.is_ok()); // deny replaced by editor → allowed again

    // ...but a team-level deny vetoes even a positive user grant.
    let team = db
        .create_team(&ctx_a, "secret", "Secret")
        .await
        .expect("team");
    db.add_team_member(&ctx_a, team.id, ctx_member.user_id)
        .await
        .expect("add team member");
    db.grant_document(&ctx_a, doc.id, "team", team.id, Role::None)
        .await
        .expect("team deny");
    assert!(
        matches!(
            db.get_document(&ctx_member, doc.id).await,
            Err(mdm_core::Error::Forbidden)
        ),
        "team deny must veto the positive user grant"
    );
    assert!(
        db.get_document(&ctx_a, doc.id).await.is_ok(),
        "admin still overrides team deny"
    );

    // a denied document is also hidden from the member's listings + search
    let member_docs = db.list_documents(&ctx_member, proj.id, 100).await.unwrap();
    assert!(
        !member_docs.iter().any(|d| d.id == doc.id),
        "denied doc hidden from member list"
    );
    let admin_docs = db.list_documents(&ctx_a, proj.id, 100).await.unwrap();
    assert!(
        admin_docs.iter().any(|d| d.id == doc.id),
        "admin still lists the doc"
    );
    assert!(
        db.search(&ctx_member, None, "hello", 10)
            .await
            .unwrap()
            .is_empty(),
        "denied doc hidden from member search"
    );
    assert!(
        db.search(&ctx_a, None, "hello", 10)
            .await
            .unwrap()
            .iter()
            .any(|h| h.document_id == doc.id),
        "admin search still finds the doc"
    );

    // cross-org: org B cannot create teams visible to A, nor grant on A's docs
    assert!(db.list_teams(&ctx_b).await.unwrap().is_empty());

    // --- public share links --------------------------------------------------
    let share = db
        .create_share_link(&ctx_a, doc.id, Some(7))
        .await
        .expect("create share");
    assert!(share.token.starts_with("sl_"));
    // public resolve returns the document content (no auth context)
    let shared = db.resolve_share_link(&share.token).await.expect("resolve");
    assert_eq!(shared.document_id, doc.id);
    assert!(!shared.content.is_empty());
    assert_eq!(db.list_share_links(&ctx_a, doc.id).await.unwrap().len(), 1);
    // a bogus token resolves to NotFound (no leak)
    assert!(matches!(
        db.resolve_share_link("sl_deadbeef00").await,
        Err(mdm_core::Error::NotFound)
    ));
    // a viewer can't mint a share link (needs editor on the doc)
    assert!(matches!(
        db.create_share_link(&ctx_viewer, doc.id, None).await,
        Err(mdm_core::Error::Forbidden)
    ));
    // revoke → the link stops resolving
    db.revoke_share_link(&ctx_a, share.info.id)
        .await
        .expect("revoke");
    assert!(matches!(
        db.resolve_share_link(&share.token).await,
        Err(mdm_core::Error::NotFound)
    ));

    // --- list documents by tag ----------------------------------------------
    db.add_document_tag(&ctx_a, doc.id, "runbook")
        .await
        .expect("tag");
    let tagged = db
        .list_documents_with_tag(&ctx_a, "runbook", 50)
        .await
        .unwrap();
    assert!(
        tagged.iter().any(|d| d.id == doc.id),
        "tag filter returns the tagged doc"
    );
    assert!(
        db.list_documents_with_tag(&ctx_a, "nope", 50)
            .await
            .unwrap()
            .is_empty(),
        "unknown tag yields nothing"
    );

    // --- audit log (admin reads who-did-what) --------------------------------
    let audit = db.list_audit(&ctx_a, None, None, 200).await.expect("audit");
    assert!(
        audit.iter().any(|e| e.action == "doc.create"),
        "doc.create audited"
    );
    assert!(
        audit.iter().any(|e| e.action.starts_with("share.")),
        "share events audited"
    );
    let doc_events = db
        .list_audit(&ctx_a, None, Some("doc."), 200)
        .await
        .unwrap();
    assert!(
        !doc_events.is_empty() && doc_events.iter().all(|e| e.action.starts_with("doc.")),
        "action-prefix filter works"
    );
    // a non-admin (viewer) cannot read the audit log
    assert!(matches!(
        db.list_audit(&ctx_viewer, None, None, 10).await,
        Err(mdm_core::Error::Forbidden)
    ));

    // --- per-project document quota (abuse guard) ----------------------------
    let qdb = connect_with_quota(2).await;
    let (_q_org, _q_user, q_key) = qdb
        .bootstrap("q@example.com", "Q", "quota", "Quota", "q")
        .await
        .expect("bootstrap quota org");
    let q_ctx = qdb
        .authenticate_api_key(&q_key.secret)
        .await
        .expect("auth quota");
    let q_proj = qdb
        .create_project(&q_ctx, "p", "P")
        .await
        .expect("quota project");
    qdb.create_document(&q_ctx, q_proj.id, "a", "A", "x")
        .await
        .expect("doc 1");
    qdb.create_document(&q_ctx, q_proj.id, "b", "B", "x")
        .await
        .expect("doc 2");
    assert!(
        matches!(
            qdb.create_document(&q_ctx, q_proj.id, "c", "C", "x").await,
            Err(mdm_core::Error::TooManyRequests(_))
        ),
        "3rd doc must exceed the per-project quota of 2"
    );

    // --- semantic + hybrid search (pgvector) — opt-in via MDM_TEST_SUPERUSER_URL ----
    // CREATE EXTENSION needs a superuser (and the schema reset dropped it), so this block
    // runs only when a superuser URL is provided.
    if let Ok(super_url) = std::env::var("MDM_TEST_SUPERUSER_URL") {
        let su = sqlx::PgPool::connect(&super_url)
            .await
            .expect("connect superuser");
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&su)
            .await
            .expect("create extension");
        su.close().await;

        let store = EmbeddingStore::connect(&owner_url(), 3)
            .await
            .expect("embedding store");
        let sdoc = db
            .create_document(&ctx_a, proj.id, "vec/doc", "Vec", "alpha beta gamma")
            .await
            .expect("vec doc");

        // Embed every pending chunk: the "alpha" one as [1,0,0], everything else as [0,1,0].
        let pending = store.pending(1000).await.expect("pending");
        assert!(
            pending.iter().any(|(_, t)| t.contains("alpha")),
            "new chunk pending"
        );
        for (cid, text) in &pending {
            let v: [f32; 3] = if text.contains("alpha") {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            store.store(*cid, &v).await.expect("store embedding");
        }

        // A query near [1,0,0] must rank the alpha doc first (tenant-scoped to org A).
        let hits = db
            .semantic_search(&ctx_a, None, &[0.9, 0.1, 0.0], 5)
            .await
            .expect("semantic");
        assert_eq!(
            hits.first().map(|h| h.document_id),
            Some(sdoc.id),
            "nearest doc is alpha"
        );

        let hybrid = db
            .hybrid_search(&ctx_a, None, "alpha", &[0.9, 0.1, 0.0], 5)
            .await
            .expect("hybrid");
        assert!(
            hybrid.iter().any(|h| h.document_id == sdoc.id),
            "hybrid finds the alpha doc"
        );

        // Dedup: editing one section preserves the unchanged section's embedding.
        let pdoc = db
            .create_document(
                &ctx_a,
                proj.id,
                "diff/doc",
                "Diff",
                "# A\nalpha one\n\n# B\nbeta two\n",
            )
            .await
            .expect("diff doc");
        for (cid, _t) in store.pending(1000).await.unwrap() {
            store.store(cid, &[0.5, 0.5, 0.0]).await.unwrap();
        }
        assert_eq!(
            store.embedded_count(pdoc.id).await.unwrap(),
            2,
            "both chunks embedded"
        );
        db.update_document(
            &ctx_a,
            pdoc.id,
            "# A\nalpha one\n\n# B\nbeta TWO changed\n",
            pdoc.current_version,
            VersionKind::Checkpoint,
        )
        .await
        .expect("edit section B");
        assert_eq!(
            store.embedded_count(pdoc.id).await.unwrap(),
            1,
            "editing section B keeps section A's embedding (only B re-queued)"
        );
        eprintln!("✓ pgvector semantic + hybrid + embedding-dedup verified");
    } else {
        eprintln!("• skipping pgvector test (set MDM_TEST_SUPERUSER_URL to run)");
    }
}
