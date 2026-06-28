//! md-manager CLI — binary `mdm`. An HTTP client over the API for humans and
//! shell-capable agents. `mdm doc get` prints raw markdown to stdout so agents can pipe
//! it straight into context.

mod config;

use std::io::{IsTerminal, Read};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use mdm_client::{Client, UpdateResult};
use serde_json::Value;

const DEFAULT_API_URL: &str = "http://127.0.0.1:8787";

#[derive(Parser)]
#[command(
    name = "mdm",
    version,
    about = "md-manager — manage markdown docs for AI agents"
)]
struct Cli {
    /// API base URL (env: MDM_API_URL; or `mdm auth login`)
    #[arg(long, global = true)]
    api_url: Option<String>,
    /// API key `mk_…` (env: MDM_API_KEY; or `mdm auth login`)
    #[arg(long, global = true)]
    api_key: Option<String>,
    /// Emit raw JSON instead of human/raw output
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new org + user + initial admin API key (needs the bootstrap token)
    Bootstrap(BootstrapArgs),
    /// Manage saved credentials
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Show the authenticated identity
    Whoami,
    /// List organizations
    Org {
        #[command(subcommand)]
        cmd: OrgCmd,
    },
    /// Projects
    Proj {
        #[command(subcommand)]
        cmd: ProjCmd,
    },
    /// Documents
    Doc {
        #[command(subcommand)]
        cmd: DocCmd,
    },
    /// Full-text search
    Search(SearchArgs),
    /// Tags
    Tag {
        #[command(subcommand)]
        cmd: TagCmd,
    },
    /// Categories (org-scoped, hierarchical)
    Cat {
        #[command(subcommand)]
        cmd: CatCmd,
    },
    /// Teams
    Team {
        #[command(subcommand)]
        cmd: TeamCmd,
    },
    /// Grants (project/document access)
    Grant {
        #[command(subcommand)]
        cmd: GrantCmd,
    },
    /// API keys
    Keys {
        #[command(subcommand)]
        cmd: KeysCmd,
    },
}

#[derive(Args)]
struct BootstrapArgs {
    #[arg(long)]
    email: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    org_slug: String,
    #[arg(long)]
    org_name: String,
    #[arg(long, default_value = "default")]
    key_name: String,
    /// Bootstrap token (env: MDM_BOOTSTRAP_TOKEN)
    #[arg(long)]
    token: Option<String>,
    /// Save the returned API URL + key to the CLI config
    #[arg(long)]
    save: bool,
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Save the API URL + key to the config file
    Login {
        #[arg(long)]
        api_url: Option<String>,
        #[arg(long)]
        api_key: String,
    },
    /// Show the authenticated identity
    Status,
    /// Remove saved credentials
    Logout,
}

#[derive(Subcommand)]
enum OrgCmd {
    List,
}

#[derive(Subcommand)]
enum ProjCmd {
    List,
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum DocCmd {
    /// List documents in a project
    List {
        #[arg(long)]
        project: String,
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Print a document's raw markdown (or --json)
    Get { id: String },
    /// Get a document by project + path
    GetPath {
        #[arg(long)]
        project: String,
        #[arg(long)]
        path: String,
    },
    /// Create a document
    Create {
        #[arg(long)]
        project: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        title: String,
        #[command(flatten)]
        body: BodyArgs,
    },
    /// Replace a document's content (optimistic concurrency)
    Edit {
        id: String,
        /// Expected current version; if omitted, fetched automatically
        #[arg(long)]
        expected_version: Option<i64>,
        #[arg(long, default_value = "checkpoint")]
        kind: String,
        #[command(flatten)]
        body: BodyArgs,
    },
    /// Append to a document
    Append {
        id: String,
        #[command(flatten)]
        body: BodyArgs,
    },
    /// Move (rename) a document
    Mv { id: String, new_path: String },
    /// Soft-delete a document
    Rm { id: String },
    /// Restore a document to a prior version
    Restore {
        id: String,
        #[arg(long)]
        version: i64,
    },
    /// Show version history
    History { id: String },
}

#[derive(Args)]
struct BodyArgs {
    /// Read content from a file
    #[arg(long)]
    file: Option<String>,
    /// Inline content
    #[arg(short = 'm', long)]
    message: Option<String>,
}

impl BodyArgs {
    fn read(&self) -> Result<String> {
        if let Some(f) = &self.file {
            return std::fs::read_to_string(f).with_context(|| format!("reading {f}"));
        }
        if let Some(m) = &self.message {
            return Ok(m.clone());
        }
        if !std::io::stdin().is_terminal() {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            return Ok(s);
        }
        bail!("no content: pass --file <path>, -m <text>, or pipe via stdin")
    }
}

#[derive(Args)]
struct SearchArgs {
    query: String,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    limit: Option<i64>,
}

#[derive(Subcommand)]
enum TagCmd {
    List,
    Add { doc_id: String, name: String },
}

#[derive(Subcommand)]
enum CatCmd {
    /// List categories
    List,
    /// Create a category (optionally under a parent)
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        parent: Option<String>,
    },
    /// File a document under a category
    Add { doc_id: String, category_id: String },
    /// List documents in a category
    Docs { category_id: String },
}

#[derive(Subcommand)]
enum TeamCmd {
    List,
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        name: String,
    },
    /// Add an org member to a team
    AddMember {
        team_id: String,
        user_id: String,
    },
}

#[derive(Subcommand)]
enum GrantCmd {
    /// Grant a user/team a role on a project (role: viewer|commenter|editor|admin)
    Project {
        project_id: String,
        #[arg(long, value_parser = ["user", "team"])]
        subject: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        role: String,
    },
    /// Grant (role: …) or DENY (role: none) a user/team on a document
    Doc {
        doc_id: String,
        #[arg(long, value_parser = ["user", "team"])]
        subject: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        role: String,
    },
}

#[derive(Subcommand)]
enum KeysCmd {
    List,
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "member")]
        role: String,
    },
    Revoke {
        id: String,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn resolve_creds(cli: &Cli) -> (String, Option<String>) {
    let cfg = config::load();
    let url = cli
        .api_url
        .clone()
        .or_else(|| std::env::var("MDM_API_URL").ok())
        .or(cfg.api_url)
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());
    let key = cli
        .api_key
        .clone()
        .or_else(|| std::env::var("MDM_API_KEY").ok())
        .or(cfg.api_key);
    (url, key)
}

fn client(cli: &Cli) -> Result<Client> {
    let (url, key) = resolve_creds(cli);
    let key = key.ok_or_else(|| anyhow!("no API key — run `mdm auth login` or set MDM_API_KEY"))?;
    Ok(Client::new(url, key))
}

fn print_json(v: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
    );
}

async fn resolve_project(c: &Client, p: &str) -> Result<String> {
    if uuid::Uuid::parse_str(p).is_ok() {
        return Ok(p.to_string());
    }
    let v = c.get_project(p).await?;
    v["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("project '{p}' not found"))
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.cmd {
        Cmd::Bootstrap(a) => {
            let (url, _) = resolve_creds(&cli);
            let token = a
                .token
                .clone()
                .or_else(|| std::env::var("MDM_BOOTSTRAP_TOKEN").ok())
                .ok_or_else(|| {
                    anyhow!("bootstrap token required (--token or MDM_BOOTSTRAP_TOKEN)")
                })?;
            let c = Client::new(url.clone(), String::new());
            let v = c
                .bootstrap(
                    &token,
                    &a.email,
                    &a.name,
                    &a.org_slug,
                    &a.org_name,
                    &a.key_name,
                )
                .await?;
            let secret = v["api_key"]["secret"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            if a.save {
                config::save(&config::StoredConfig {
                    api_url: Some(url),
                    api_key: Some(secret.clone()),
                })?;
                eprintln!("saved credentials to {}", config::config_path().display());
            }
            if cli.json {
                print_json(&v);
            } else {
                println!("org:     {}", v["org"]["slug"].as_str().unwrap_or(""));
                println!("api key: {secret}");
                eprintln!("(store this key now — it is shown only once)");
            }
        }

        Cmd::Auth { cmd } => match cmd {
            AuthCmd::Login { api_url, api_key } => {
                let url = api_url
                    .clone()
                    .or_else(|| std::env::var("MDM_API_URL").ok())
                    .unwrap_or_else(|| DEFAULT_API_URL.to_string());
                let path = config::save(&config::StoredConfig {
                    api_url: Some(url),
                    api_key: Some(api_key.clone()),
                })?;
                println!("saved credentials to {}", path.display());
            }
            AuthCmd::Status => {
                print_json(&client(&cli)?.whoami().await?);
            }
            AuthCmd::Logout => {
                let path = config::config_path();
                let _ = std::fs::remove_file(&path);
                println!("removed {}", path.display());
            }
        },

        Cmd::Whoami => print_json(&client(&cli)?.whoami().await?),

        Cmd::Org { cmd } => match cmd {
            OrgCmd::List => print_json(&client(&cli)?.list_orgs().await?),
        },

        Cmd::Proj { cmd } => {
            let c = client(&cli)?;
            match cmd {
                ProjCmd::List => print_json(&c.list_projects().await?),
                ProjCmd::Create { slug, name } => print_json(&c.create_project(slug, name).await?),
            }
        }

        Cmd::Doc { cmd } => {
            let c = client(&cli)?;
            match cmd {
                DocCmd::List { project, limit } => {
                    let pid = resolve_project(&c, project).await?;
                    print_json(&c.list_documents(&pid, *limit).await?);
                }
                DocCmd::Get { id } => {
                    let v = c.get_document(id).await?;
                    if cli.json {
                        print_json(&v);
                    } else {
                        print!("{}", v["content"].as_str().unwrap_or_default());
                    }
                }
                DocCmd::GetPath { project, path } => {
                    let pid = resolve_project(&c, project).await?;
                    let v = c.get_document_by_path(&pid, path).await?;
                    if cli.json {
                        print_json(&v);
                    } else {
                        print!("{}", v["content"].as_str().unwrap_or_default());
                    }
                }
                DocCmd::Create {
                    project,
                    path,
                    title,
                    body,
                } => {
                    let pid = resolve_project(&c, project).await?;
                    let content = body.read()?;
                    print_json(&c.create_document(&pid, path, title, &content).await?);
                }
                DocCmd::Edit {
                    id,
                    expected_version,
                    kind,
                    body,
                } => {
                    let content = body.read()?;
                    let expected = match expected_version {
                        Some(v) => *v,
                        None => c.get_document(id).await?["current_version"]
                            .as_i64()
                            .unwrap_or(0),
                    };
                    match c.update_document(id, &content, expected, kind).await? {
                        UpdateResult::Updated(v) => {
                            if cli.json {
                                print_json(&v);
                            } else {
                                println!("updated to version {}", v["current_version"]);
                            }
                        }
                        UpdateResult::Conflict {
                            current_version, ..
                        } => {
                            bail!(
                                "conflict: document is now at version {current_version} (your base was stale). \
                                 Re-fetch with `mdm doc get {id}` and retry."
                            );
                        }
                    }
                }
                DocCmd::Append { id, body } => {
                    let content = body.read()?;
                    let v = c.append_document(id, &content).await?;
                    if cli.json {
                        print_json(&v);
                    } else {
                        println!("appended; now version {}", v["current_version"]);
                    }
                }
                DocCmd::Mv { id, new_path } => print_json(&c.move_document(id, new_path).await?),
                DocCmd::Rm { id } => {
                    c.delete_document(id).await?;
                    println!("deleted {id}");
                }
                DocCmd::Restore { id, version } => {
                    print_json(&c.restore_version(id, *version).await?)
                }
                DocCmd::History { id } => print_json(&c.history(id).await?),
            }
        }

        Cmd::Search(a) => {
            let c = client(&cli)?;
            let pid = match &a.project {
                Some(p) => Some(resolve_project(&c, p).await?),
                None => None,
            };
            let v = c.search(&a.query, pid.as_deref(), a.limit).await?;
            if cli.json {
                print_json(&v);
            } else {
                print_search(&v);
            }
        }

        Cmd::Tag { cmd } => {
            let c = client(&cli)?;
            match cmd {
                TagCmd::List => print_json(&c.list_tags().await?),
                TagCmd::Add { doc_id, name } => {
                    print_json(&c.add_document_tag(doc_id, name).await?)
                }
            }
        }

        Cmd::Cat { cmd } => {
            let c = client(&cli)?;
            match cmd {
                CatCmd::List => print_json(&c.list_categories().await?),
                CatCmd::Create { slug, name, parent } => {
                    print_json(&c.create_category(parent.as_deref(), slug, name).await?)
                }
                CatCmd::Add {
                    doc_id,
                    category_id,
                } => {
                    c.categorize_document(doc_id, category_id).await?;
                    println!("filed {doc_id} under {category_id}");
                }
                CatCmd::Docs { category_id } => {
                    print_json(&c.list_category_documents(category_id).await?)
                }
            }
        }

        Cmd::Team { cmd } => {
            let c = client(&cli)?;
            match cmd {
                TeamCmd::List => print_json(&c.list_teams().await?),
                TeamCmd::Create { slug, name } => print_json(&c.create_team(slug, name).await?),
                TeamCmd::AddMember { team_id, user_id } => {
                    c.add_team_member(team_id, user_id).await?;
                    println!("added {user_id} to team {team_id}");
                }
            }
        }

        Cmd::Grant { cmd } => {
            let c = client(&cli)?;
            match cmd {
                GrantCmd::Project {
                    project_id,
                    subject,
                    id,
                    role,
                } => {
                    c.grant_project(project_id, subject, id, role).await?;
                    println!("granted {subject}:{id} {role} on project {project_id}");
                }
                GrantCmd::Doc {
                    doc_id,
                    subject,
                    id,
                    role,
                } => {
                    c.grant_document(doc_id, subject, id, role).await?;
                    println!("granted {subject}:{id} {role} on document {doc_id}");
                }
            }
        }

        Cmd::Keys { cmd } => {
            let c = client(&cli)?;
            match cmd {
                KeysCmd::List => print_json(&c.list_api_keys().await?),
                KeysCmd::Create { name, role } => print_json(&c.create_api_key(name, role).await?),
                KeysCmd::Revoke { id } => {
                    c.revoke_api_key(id).await?;
                    println!("revoked {id}");
                }
            }
        }
    }
    Ok(())
}

fn print_search(v: &Value) {
    let Some(arr) = v.as_array() else {
        print_json(v);
        return;
    };
    if arr.is_empty() {
        println!("no matches");
        return;
    }
    for h in arr {
        println!(
            "{}  [{}]  (rank {:.3})",
            h["path"].as_str().unwrap_or(""),
            h["title"].as_str().unwrap_or(""),
            h["rank"].as_f64().unwrap_or(0.0)
        );
        println!("    id: {}", h["document_id"].as_str().unwrap_or(""));
        println!(
            "    {}",
            h["snippet"].as_str().unwrap_or("").replace('\n', " ")
        );
    }
}
