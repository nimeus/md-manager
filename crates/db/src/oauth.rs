//! Built-in OAuth 2.1 Authorization Server: client registration (DCR), authorization
//! requests, single-use codes, and rotating access/refresh tokens. Plus the resource-server
//! side: validating an opaque `mo_` access token into an [`AuthContext`].
//!
//! All five tables are RLS-exempt (like `api_keys` / `share_links`): clients and pending
//! requests exist before an org is chosen, and tokens are looked up by prefix before the org
//! context exists. Secrets are stored as HMAC-SHA256(pepper, secret) + a lookup prefix and
//! verified in constant time. Codes/tokens carry `(user_id, org_id)`; the membership role is
//! re-resolved on every use, so a token dies if the user loses the org.

use mdm_core::model::{ActorType, AuthContext, OrgRole};
use mdm_core::{Error, Result, crypto, ids, oauth as core_oauth};
use sqlx::{FromRow, Postgres, Transaction};
use serde::Serialize;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Db, audit, map_db};

/// A freshly registered client; `client_secret` is `None` for public (PKCE-only) clients.
#[derive(Debug, Clone)]
pub struct RegisteredOAuthClient {
    pub client_id: String,
    pub client_secret: Option<String>,
}

/// Public, non-secret view of a registered client.
#[derive(Debug, Clone)]
pub struct OAuthClientInfo {
    pub db_id: Uuid,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub is_public: bool,
    pub scopes: String,
}

/// Consent-page display info for a pending authorization request.
#[derive(Debug, Clone)]
pub struct AuthRequestDisplay {
    pub client_name: String,
    pub scope: String,
    pub redirect_uri: String,
}

/// Result of approving consent: where to send the browser, with the minted code + echoed state.
#[derive(Debug, Clone)]
pub struct MintedCode {
    pub redirect_uri: String,
    pub code: String,
    pub state: Option<String>,
}

/// Result of denying consent.
#[derive(Debug, Clone)]
pub struct DenyOutcome {
    pub redirect_uri: String,
    pub state: Option<String>,
}

/// An issued token pair (returned by the token endpoint).
#[derive(Debug, Clone)]
pub struct IssuedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_in: i64,
    pub scope: String,
    pub resource: String,
}

/// What an `mo_` access token resolves to: a single bound org, or all the user's orgs (the
/// connector then selects the org per call via the MCP `org` argument).
#[derive(Debug, Clone)]
pub enum OAuthAccess {
    Org(AuthContext),
    AllOrgs { user_id: Uuid },
}

/// A user's connected app (one connector grant in one org) for the "Connected apps" panel.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct OAuthGrant {
    pub client_name: String,
    pub client_id: String,
    pub org_id: Uuid,
    pub org_name: String,
    pub all_orgs: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub connected_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

#[derive(FromRow)]
struct ClientRow {
    id: Uuid,
    client_id: String,
    client_secret_hash: Option<String>,
    name: String,
    redirect_uris: Vec<String>,
    scopes: String,
    revoked_at: Option<OffsetDateTime>,
}

impl ClientRow {
    fn into_info(self) -> OAuthClientInfo {
        OAuthClientInfo {
            db_id: self.id,
            client_id: self.client_id,
            name: self.name,
            redirect_uris: self.redirect_uris,
            is_public: self.client_secret_hash.is_none(),
            scopes: self.scopes,
        }
    }
}

const CLIENT_COLS: &str =
    "id, client_id, client_secret_hash, name, redirect_uris, scopes, revoked_at";

#[derive(FromRow)]
struct AuthRequestRow {
    client_id: Uuid,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    resource: String,
    scope: String,
    state: Option<String>,
}

#[derive(FromRow)]
struct CodeRow {
    id: Uuid,
    code_hash: String,
    client_id: Uuid,
    user_id: Uuid,
    org_id: Uuid,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    resource: String,
    scope: String,
    all_orgs: bool,
}

#[derive(FromRow)]
struct AccessTokenRow {
    id: Uuid,
    token_hash: String,
    user_id: Uuid,
    org_id: Uuid,
    resource: String,
    expires_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    all_orgs: bool,
}

#[derive(FromRow)]
struct RefreshRow {
    id: Uuid,
    token_hash: String,
    client_id: Uuid,
    user_id: Uuid,
    org_id: Uuid,
    scope: String,
    resource: String,
    expires_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    rotated_to: Option<Uuid>,
    all_orgs: bool,
}

impl Db {
    // ── Client registration (DCR) ────────────────────────────────────────────────────────

    /// Register a client (RFC 7591). Public clients (Claude/ChatGPT) get no secret — PKCE is
    /// their sole proof at the token endpoint.
    pub async fn register_oauth_client(
        &self,
        name: &str,
        redirect_uris: &[String],
        public: bool,
    ) -> Result<RegisteredOAuthClient> {
        let client_id = core_oauth::generate_client_id();
        let (secret, secret_prefix, secret_hash) = if public {
            (None, None, None)
        } else {
            let s = core_oauth::generate_client_secret();
            let h = crypto::hash_token(&self.pepper, &s.secret);
            (Some(s.secret), Some(s.prefix), Some(h))
        };
        sqlx::query(
            "INSERT INTO oauth_clients
               (id, client_id, client_secret_prefix, client_secret_hash, name, redirect_uris, is_dcr)
             VALUES ($1, $2, $3, $4, $5, $6, true)",
        )
        .bind(ids::new_id())
        .bind(&client_id)
        .bind(&secret_prefix)
        .bind(&secret_hash)
        .bind(name)
        .bind(redirect_uris)
        .execute(self.pool())
        .await
        .map_err(map_db)?;

        Ok(RegisteredOAuthClient {
            client_id,
            client_secret: secret,
        })
    }

    /// Look up a client by its public `client_id` (for the authorize endpoint).
    pub async fn find_oauth_client(&self, client_id: &str) -> Result<OAuthClientInfo> {
        let row = sqlx::query_as::<_, ClientRow>(&format!(
            "SELECT {CLIENT_COLS} FROM oauth_clients WHERE client_id = $1"
        ))
        .bind(client_id)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?
        .ok_or(Error::Unauthorized)?;
        if row.revoked_at.is_some() {
            return Err(Error::Unauthorized);
        }
        Ok(row.into_info())
    }

    /// Authenticate a client at the token endpoint. Confidential clients must present a valid
    /// secret; public clients authenticate by `client_id` alone (PKCE is the proof).
    pub async fn authenticate_oauth_client(
        &self,
        client_id: &str,
        secret: Option<&str>,
    ) -> Result<OAuthClientInfo> {
        let row = sqlx::query_as::<_, ClientRow>(&format!(
            "SELECT {CLIENT_COLS} FROM oauth_clients WHERE client_id = $1"
        ))
        .bind(client_id)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?
        .ok_or(Error::Unauthorized)?;
        if row.revoked_at.is_some() {
            return Err(Error::Unauthorized);
        }
        if let Some(hash) = &row.client_secret_hash {
            // Confidential client: a valid secret is required.
            let s = secret.ok_or(Error::Unauthorized)?;
            if !crypto::verify_token(&self.pepper, s, hash) {
                return Err(Error::Unauthorized);
            }
        }
        Ok(row.into_info())
    }

    // ── Authorization request → consent → code ───────────────────────────────────────────

    /// Store a validated authorization request (the handler has already checked the client +
    /// redirect_uri + PKCE method). Returns the request id used to build the consent URL.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_authorization_request(
        &self,
        client_db_id: Uuid,
        redirect_uri: &str,
        code_challenge: &str,
        code_challenge_method: &str,
        resource: &str,
        scope: &str,
        state: Option<&str>,
        ttl_secs: i64,
    ) -> Result<Uuid> {
        let id = ids::new_id();
        let expires_at = OffsetDateTime::now_utc() + Duration::seconds(ttl_secs);
        sqlx::query(
            "INSERT INTO oauth_authorization_requests
               (id, client_id, redirect_uri, code_challenge, code_challenge_method,
                resource, scope, state, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(client_db_id)
        .bind(redirect_uri)
        .bind(code_challenge)
        .bind(code_challenge_method)
        .bind(resource)
        .bind(scope)
        .bind(state)
        .bind(expires_at)
        .execute(self.pool())
        .await
        .map_err(map_db)?;
        Ok(id)
    }

    /// Consent-page display info for a pending (unconsumed, unexpired) request.
    pub async fn get_authorization_request_display(&self, id: Uuid) -> Result<AuthRequestDisplay> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT c.name, r.scope, r.redirect_uri
             FROM oauth_authorization_requests r
             JOIN oauth_clients c ON c.id = r.client_id
             WHERE r.id = $1 AND r.consumed_at IS NULL AND r.expires_at > now()",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?;
        Ok(AuthRequestDisplay {
            client_name: row.0,
            scope: row.1,
            redirect_uri: row.2,
        })
    }

    /// User approved consent: atomically consume the request, verify the user is a member of
    /// the chosen org, and mint a single-use code bound to (client, user, org, redirect, pkce,
    /// resource, scope). The `user_id` comes from the verified session — never the request body.
    pub async fn approve_authorization_request(
        &self,
        request_id: Uuid,
        user_id: Uuid,
        org_id: Option<Uuid>,
        all_orgs: bool,
        code_ttl_secs: i64,
    ) -> Result<MintedCode> {
        // The org we scope to + bind into the code: the chosen org, or (for all-orgs) the user's
        // home org as a placeholder. For all-orgs the bound org isn't authoritative — the agent
        // selects the org per call.
        let scope_org = if all_orgs {
            self.list_user_orgs(user_id)
                .await?
                .first()
                .map(|o| o.id)
                .ok_or(Error::Forbidden)?
        } else {
            org_id.ok_or_else(|| Error::invalid("org_id is required"))?
        };
        let mut tx = self
            .begin_scoped(scope_org, user_id, ActorType::User)
            .await
            .map_err(map_db)?;

        // Atomic single-use consume of the pending request.
        let req = sqlx::query_as::<_, AuthRequestRow>(
            "UPDATE oauth_authorization_requests SET consumed_at = now()
             WHERE id = $1 AND consumed_at IS NULL AND expires_at > now()
             RETURNING client_id, redirect_uri, code_challenge, code_challenge_method,
                       resource, scope, state",
        )
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?;

        // The consenting user must currently be a member of the scoped org.
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(scope_org)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let role = OrgRole::from_db(&role.ok_or(Error::Forbidden)?)?;

        let code = core_oauth::generate_auth_code();
        let code_hash = crypto::hash_token(&self.pepper, &code.secret);
        let expires_at = OffsetDateTime::now_utc() + Duration::seconds(code_ttl_secs);
        sqlx::query(
            "INSERT INTO oauth_auth_codes
               (id, code_prefix, code_hash, client_id, user_id, org_id, redirect_uri,
                code_challenge, code_challenge_method, resource, scope, expires_at, all_orgs)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(ids::new_id())
        .bind(&code.prefix)
        .bind(&code_hash)
        .bind(req.client_id)
        .bind(user_id)
        .bind(scope_org)
        .bind(&req.redirect_uri)
        .bind(&req.code_challenge)
        .bind(&req.code_challenge_method)
        .bind(&req.resource)
        .bind(&req.scope)
        .bind(expires_at)
        .bind(all_orgs)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;

        audit(
            &mut tx,
            &AuthContext {
                org_id: scope_org,
                user_id,
                actor_type: ActorType::User,
                org_role: role,
            },
            "oauth.grant",
            Some(&req.client_id.to_string()),
            serde_json::json!({ "scope": req.scope, "all_orgs": all_orgs }),
        )
        .await
        .map_err(map_db)?;

        tx.commit().await.map_err(map_db)?;
        Ok(MintedCode {
            redirect_uri: req.redirect_uri,
            code: code.secret,
            state: req.state,
        })
    }

    /// User denied consent: consume the request, return where to send the `access_denied` error.
    pub async fn deny_authorization_request(&self, request_id: Uuid) -> Result<DenyOutcome> {
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "UPDATE oauth_authorization_requests SET consumed_at = now()
             WHERE id = $1 AND consumed_at IS NULL AND expires_at > now()
             RETURNING redirect_uri, state",
        )
        .bind(request_id)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?;
        Ok(DenyOutcome {
            redirect_uri: row.0,
            state: row.1,
        })
    }

    // ── Token endpoint ───────────────────────────────────────────────────────────────────

    /// `authorization_code` grant: atomically consume the code, verify it belongs to this
    /// client, the redirect_uri matches exactly, PKCE verifies (S256), and the audience matches.
    /// Issues an access + refresh pair. Any failure consumes the code (single-use) and returns
    /// `Unauthorized` (the handler maps that to `invalid_grant`).
    #[allow(clippy::too_many_arguments)]
    pub async fn exchange_auth_code(
        &self,
        client_db_id: Uuid,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
        resource: Option<&str>,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Result<IssuedTokens> {
        let prefix =
            crypto::token_prefix(core_oauth::AUTH_CODE_SCHEME, code).ok_or(Error::Unauthorized)?;
        let mut tx = self.pool().begin().await.map_err(map_db)?;

        let candidates = sqlx::query_as::<_, CodeRow>(
            "SELECT id, code_hash, client_id, user_id, org_id, redirect_uri,
                    code_challenge, code_challenge_method, resource, scope, all_orgs
             FROM oauth_auth_codes WHERE code_prefix = $1 FOR UPDATE",
        )
        .bind(&prefix)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        let matched = candidates
            .into_iter()
            .find(|r| crypto::verify_token(&self.pepper, code, &r.code_hash))
            .ok_or(Error::Unauthorized)?;

        // Atomic single-use consume by id (guards double-spend even under concurrency).
        let consumed = sqlx::query(
            "UPDATE oauth_auth_codes SET consumed_at = now()
             WHERE id = $1 AND consumed_at IS NULL AND expires_at > now()",
        )
        .bind(matched.id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?
        .rows_affected();
        if consumed == 0 {
            return Err(Error::Unauthorized); // already used or expired
        }

        // Bindings — any mismatch is invalid_grant.
        if matched.client_id != client_db_id
            || matched.redirect_uri != redirect_uri
            || matched.code_challenge_method != core_oauth::PKCE_METHOD_S256
            || !core_oauth::verify_pkce_s256(code_verifier, &matched.code_challenge)
        {
            return Err(Error::Unauthorized);
        }
        if let Some(r) = resource
            && r != matched.resource
        {
            return Err(Error::Unauthorized);
        }

        let tokens = self
            .issue_token_pair(
                &mut tx,
                client_db_id,
                matched.user_id,
                matched.org_id,
                &matched.scope,
                &matched.resource,
                matched.all_orgs,
                access_ttl_secs,
                refresh_ttl_secs,
            )
            .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(tokens)
    }

    /// `refresh_token` grant: rotate the refresh token. Presenting an already-rotated or
    /// revoked refresh is a theft signal → revoke the whole `(client,user,org)` family.
    pub async fn refresh_oauth_token(
        &self,
        client_db_id: Uuid,
        refresh_token: &str,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Result<IssuedTokens> {
        let prefix = crypto::token_prefix(core_oauth::REFRESH_TOKEN_SCHEME, refresh_token)
            .ok_or(Error::Unauthorized)?;
        let mut tx = self.pool().begin().await.map_err(map_db)?;

        let candidates = sqlx::query_as::<_, RefreshRow>(
            "SELECT id, token_hash, client_id, user_id, org_id, scope, resource,
                    expires_at, revoked_at, rotated_to, all_orgs
             FROM oauth_refresh_tokens WHERE token_prefix = $1 FOR UPDATE",
        )
        .bind(&prefix)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        let matched = candidates
            .into_iter()
            .find(|r| crypto::verify_token(&self.pepper, refresh_token, &r.token_hash))
            .ok_or(Error::Unauthorized)?;

        if matched.client_id != client_db_id {
            return Err(Error::Unauthorized);
        }
        // Reuse detection: a refresh that's already rotated/revoked → kill the family.
        if matched.revoked_at.is_some() || matched.rotated_to.is_some() {
            revoke_family(&mut tx, matched.client_id, matched.user_id, matched.org_id).await?;
            tx.commit().await.map_err(map_db)?;
            return Err(Error::Unauthorized);
        }
        if matched.expires_at <= OffsetDateTime::now_utc() {
            return Err(Error::Unauthorized);
        }

        let (tokens, new_refresh_id) = self
            .issue_token_pair_with_ids(
                &mut tx,
                matched.client_id,
                matched.user_id,
                matched.org_id,
                &matched.scope,
                &matched.resource,
                matched.all_orgs,
                access_ttl_secs,
                refresh_ttl_secs,
            )
            .await?;

        // Mark the old refresh rotated (guarded against a concurrent rotation).
        let rotated = sqlx::query(
            "UPDATE oauth_refresh_tokens SET revoked_at = now(), rotated_to = $2
             WHERE id = $1 AND revoked_at IS NULL AND rotated_to IS NULL",
        )
        .bind(matched.id)
        .bind(new_refresh_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?
        .rows_affected();
        if rotated == 0 {
            return Err(Error::Unauthorized); // concurrent rotation → drop (rollback) the new pair
        }
        tx.commit().await.map_err(map_db)?;
        Ok(tokens)
    }

    /// RFC 7009 token revocation. Accepts an access or refresh token; revoking a refresh kills
    /// its whole family. Always succeeds (unknown tokens are a no-op).
    pub async fn revoke_oauth_token(&self, token: &str) -> Result<()> {
        if let Some(prefix) = crypto::token_prefix(core_oauth::ACCESS_TOKEN_SCHEME, token) {
            let candidates =
                sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT id, token_hash FROM oauth_access_tokens WHERE token_prefix = $1",
                )
                .bind(&prefix)
                .fetch_all(self.pool())
                .await
                .map_err(map_db)?;
            if let Some((id, _)) = candidates
                .into_iter()
                .find(|(_, h)| crypto::verify_token(&self.pepper, token, h))
            {
                sqlx::query(
                    "UPDATE oauth_access_tokens SET revoked_at = now()
                     WHERE id = $1 AND revoked_at IS NULL",
                )
                .bind(id)
                .execute(self.pool())
                .await
                .map_err(map_db)?;
            }
        } else if let Some(prefix) = crypto::token_prefix(core_oauth::REFRESH_TOKEN_SCHEME, token) {
            let candidates = sqlx::query_as::<_, (String, Uuid, Uuid, Uuid)>(
                "SELECT token_hash, client_id, user_id, org_id
                 FROM oauth_refresh_tokens WHERE token_prefix = $1",
            )
            .bind(&prefix)
            .fetch_all(self.pool())
            .await
            .map_err(map_db)?;
            if let Some((_, client_id, user_id, org_id)) = candidates
                .into_iter()
                .find(|(h, ..)| crypto::verify_token(&self.pepper, token, h))
            {
                let mut tx = self.pool().begin().await.map_err(map_db)?;
                revoke_family(&mut tx, client_id, user_id, org_id).await?;
                tx.commit().await.map_err(map_db)?;
            }
        }
        Ok(())
    }

    // ── Resource server: validate an access token into a request context ─────────────────

    /// Validate an opaque `mo_` access token (sent to /mcp). Looks it up by prefix, verifies
    /// the hash in constant time, checks expiry/revocation and audience, then re-resolves the
    /// user's CURRENT membership role in the bound org (so the token dies if they lose it).
    /// The org is intrinsic to the token — callers must NOT pass an org override.
    pub async fn authenticate_oauth_access_token(
        &self,
        secret: &str,
        expected_resource: &str,
    ) -> Result<OAuthAccess> {
        let prefix = crypto::token_prefix(core_oauth::ACCESS_TOKEN_SCHEME, secret)
            .ok_or(Error::Unauthorized)?;
        let candidates = sqlx::query_as::<_, AccessTokenRow>(
            "SELECT id, token_hash, user_id, org_id, resource, expires_at, revoked_at, all_orgs
             FROM oauth_access_tokens WHERE token_prefix = $1",
        )
        .bind(&prefix)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;

        let now = OffsetDateTime::now_utc();
        let token = candidates
            .into_iter()
            .find(|r| crypto::verify_token(&self.pepper, secret, &r.token_hash))
            .ok_or(Error::Unauthorized)?;
        if token.revoked_at.is_some()
            || token.expires_at <= now
            || token.resource != expected_resource
        {
            return Err(Error::Unauthorized);
        }
        // Best-effort last-used bookkeeping (RLS-exempt table → no scope needed).
        let _ = sqlx::query("UPDATE oauth_access_tokens SET last_used_at = now() WHERE id = $1")
            .bind(token.id)
            .execute(self.pool())
            .await;

        if token.all_orgs {
            // The connector spans all the user's orgs; membership is checked per call.
            return Ok(OAuthAccess::AllOrgs {
                user_id: token.user_id,
            });
        }

        // Single-org: re-resolve the user's CURRENT membership role in the bound org.
        let mut tx = self
            .begin_scoped(token.org_id, token.user_id, ActorType::Agent)
            .await
            .map_err(map_db)?;
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(token.org_id)
        .bind(token.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;

        let role = OrgRole::from_db(&role.ok_or(Error::Unauthorized)?)?;
        Ok(OAuthAccess::Org(AuthContext {
            org_id: token.org_id,
            user_id: token.user_id,
            actor_type: ActorType::Agent,
            org_role: role,
        }))
    }

    /// Resolve `(user, org)` for an all-orgs connector — the agent passes the org slug (or id)
    /// per call. Verifies live membership; returns the request context.
    pub async fn authenticate_oauth_user_in_org(
        &self,
        user_id: Uuid,
        org_ref: &str,
    ) -> Result<AuthContext> {
        let org = self
            .list_user_orgs(user_id)
            .await?
            .into_iter()
            .find(|o| o.slug == org_ref || o.id.to_string() == org_ref)
            .ok_or(Error::Forbidden)?;
        Ok(AuthContext {
            org_id: org.id,
            user_id,
            actor_type: ActorType::Agent,
            org_role: org.role,
        })
    }

    /// Delete expired authorization requests + codes (housekeeping; no TTL GC in Postgres).
    pub async fn cleanup_expired_oauth(&self) -> Result<u64> {
        let a = sqlx::query("DELETE FROM oauth_authorization_requests WHERE expires_at < now()")
            .execute(self.pool())
            .await
            .map_err(map_db)?
            .rows_affected();
        let b = sqlx::query("DELETE FROM oauth_auth_codes WHERE expires_at < now()")
            .execute(self.pool())
            .await
            .map_err(map_db)?
            .rows_affected();
        Ok(a + b)
    }

    // ── Connected apps (user-facing grant management) ───────────────────────────────────

    /// List the user's active connector grants across all their orgs (one row per connected
    /// app + org), newest first. Powers the "Connected apps" panel.
    pub async fn list_oauth_grants(&self, ctx: &AuthContext) -> Result<Vec<OAuthGrant>> {
        // Scope by user so the `organizations` join is readable (org_member_read policy).
        let mut tx = self.begin_user_scoped(ctx.user_id).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, OAuthGrant>(
            "SELECT c.name AS client_name, c.client_id, o.id AS org_id, o.name AS org_name,
                    bool_or(r.all_orgs) AS all_orgs, max(r.created_at) AS connected_at,
                    (SELECT max(a.last_used_at) FROM oauth_access_tokens a
                       WHERE a.client_id = c.id AND a.user_id = $1 AND a.org_id = o.id
                         AND a.revoked_at IS NULL) AS last_used_at
             FROM oauth_refresh_tokens r
             JOIN oauth_clients c ON c.id = r.client_id
             JOIN organizations o ON o.id = r.org_id
             WHERE r.user_id = $1 AND r.revoked_at IS NULL AND r.rotated_to IS NULL
                   AND r.expires_at > now()
             GROUP BY c.id, c.name, c.client_id, o.id, o.name
             ORDER BY connected_at DESC",
        )
        .bind(ctx.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows)
    }

    /// Revoke a connected app's grant in one org (kills the whole token family there).
    pub async fn revoke_oauth_grant(
        &self,
        ctx: &AuthContext,
        client_id: &str,
        org_id: Uuid,
    ) -> Result<()> {
        let client_db_id = self.grant_client_db_id(client_id, ctx.user_id, org_id).await?;
        let mut tx = self
            .begin_scoped(org_id, ctx.user_id, ActorType::User)
            .await
            .map_err(map_db)?;
        revoke_family(&mut tx, client_db_id, ctx.user_id, org_id).await?;
        audit(
            &mut tx,
            &AuthContext {
                org_id,
                user_id: ctx.user_id,
                actor_type: ActorType::User,
                org_role: ctx.org_role,
            },
            "oauth.revoke",
            Some(client_id),
            serde_json::json!({}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    /// Move a live connection from `from_org` to `to_org` (which the user must belong to). The
    /// token strings are unchanged — the connector keeps working, now acting in the new org.
    pub async fn switch_oauth_grant(
        &self,
        ctx: &AuthContext,
        client_id: &str,
        from_org: Uuid,
        to_org: Uuid,
    ) -> Result<()> {
        if from_org == to_org {
            return Ok(());
        }
        let client_db_id = self.grant_client_db_id(client_id, ctx.user_id, from_org).await?;
        // The caller must currently be a member of the target org (RLS scope = to_org).
        let mut tx = self
            .begin_scoped(to_org, ctx.user_id, ActorType::User)
            .await
            .map_err(map_db)?;
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(to_org)
        .bind(ctx.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let role = OrgRole::from_db(&role.ok_or(Error::Forbidden)?)?;

        // Re-bind the live token family. Table names are fixed literals (injection-safe).
        for table in ["oauth_access_tokens", "oauth_refresh_tokens"] {
            sqlx::query(&format!(
                "UPDATE {table} SET org_id = $1
                 WHERE client_id = $2 AND user_id = $3 AND org_id = $4 AND revoked_at IS NULL"
            ))
            .bind(to_org)
            .bind(client_db_id)
            .bind(ctx.user_id)
            .bind(from_org)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
        }
        audit(
            &mut tx,
            &AuthContext {
                org_id: to_org,
                user_id: ctx.user_id,
                actor_type: ActorType::User,
                org_role: role,
            },
            "oauth.switch",
            Some(client_id),
            serde_json::json!({ "from_org": from_org.to_string(), "to_org": to_org.to_string() }),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    /// Resolve a public `client_id` to its db id, ensuring the user has a grant in `org_id`.
    async fn grant_client_db_id(
        &self,
        client_id: &str,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Uuid> {
        sqlx::query_scalar(
            "SELECT c.id FROM oauth_clients c WHERE c.client_id = $1 AND EXISTS (
               SELECT 1 FROM oauth_refresh_tokens r
               WHERE r.client_id = c.id AND r.user_id = $2 AND r.org_id = $3 AND r.revoked_at IS NULL)",
        )
        .bind(client_id)
        .bind(user_id)
        .bind(org_id)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)
    }

    // ── helpers ──────────────────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    async fn issue_token_pair(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        client_db_id: Uuid,
        user_id: Uuid,
        org_id: Uuid,
        scope: &str,
        resource: &str,
        all_orgs: bool,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Result<IssuedTokens> {
        let (tokens, _) = self
            .issue_token_pair_with_ids(
                tx,
                client_db_id,
                user_id,
                org_id,
                scope,
                resource,
                all_orgs,
                access_ttl_secs,
                refresh_ttl_secs,
            )
            .await?;
        Ok(tokens)
    }

    #[allow(clippy::too_many_arguments)]
    async fn issue_token_pair_with_ids(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        client_db_id: Uuid,
        user_id: Uuid,
        org_id: Uuid,
        scope: &str,
        resource: &str,
        all_orgs: bool,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Result<(IssuedTokens, Uuid)> {
        let now = OffsetDateTime::now_utc();

        let access = core_oauth::generate_access_token();
        let access_hash = crypto::hash_token(&self.pepper, &access.secret);
        let access_id = ids::new_id();
        sqlx::query(
            "INSERT INTO oauth_access_tokens
               (id, token_prefix, token_hash, client_id, user_id, org_id, scope, resource,
                all_orgs, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(access_id)
        .bind(&access.prefix)
        .bind(&access_hash)
        .bind(client_db_id)
        .bind(user_id)
        .bind(org_id)
        .bind(scope)
        .bind(resource)
        .bind(all_orgs)
        .bind(now + Duration::seconds(access_ttl_secs))
        .execute(&mut **tx)
        .await
        .map_err(map_db)?;

        let refresh = core_oauth::generate_refresh_token();
        let refresh_hash = crypto::hash_token(&self.pepper, &refresh.secret);
        let refresh_id = ids::new_id();
        sqlx::query(
            "INSERT INTO oauth_refresh_tokens
               (id, token_prefix, token_hash, client_id, user_id, org_id, scope, resource,
                access_token_id, all_orgs, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(refresh_id)
        .bind(&refresh.prefix)
        .bind(&refresh_hash)
        .bind(client_db_id)
        .bind(user_id)
        .bind(org_id)
        .bind(scope)
        .bind(resource)
        .bind(access_id)
        .bind(all_orgs)
        .bind(now + Duration::seconds(refresh_ttl_secs))
        .execute(&mut **tx)
        .await
        .map_err(map_db)?;

        Ok((
            IssuedTokens {
                access_token: access.secret,
                refresh_token: refresh.secret,
                access_expires_in: access_ttl_secs,
                scope: scope.to_string(),
                resource: resource.to_string(),
            },
            refresh_id,
        ))
    }
}

/// Revoke every un-revoked access + refresh token for a `(client, user, org)` family.
async fn revoke_family(
    tx: &mut Transaction<'_, Postgres>,
    client_id: Uuid,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "UPDATE oauth_refresh_tokens SET revoked_at = now()
         WHERE client_id = $1 AND user_id = $2 AND org_id = $3 AND revoked_at IS NULL",
    )
    .bind(client_id)
    .bind(user_id)
    .bind(org_id)
    .execute(&mut **tx)
    .await
    .map_err(map_db)?;
    sqlx::query(
        "UPDATE oauth_access_tokens SET revoked_at = now()
         WHERE client_id = $1 AND user_id = $2 AND org_id = $3 AND revoked_at IS NULL",
    )
    .bind(client_id)
    .bind(user_id)
    .bind(org_id)
    .execute(&mut **tx)
    .await
    .map_err(map_db)?;
    Ok(())
}
