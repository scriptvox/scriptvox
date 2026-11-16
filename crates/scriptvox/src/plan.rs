//! Plans and approval (PLAN-R1 §2.3): a plan lists the stages, the cost and
//! the downloads before anything runs. `plan_id` is a hash of everything that
//! decides the cost, so an approval is for exactly this plan. A $0 plan needs
//! no approval. Pattern from MoneyPrinterTurbo `mpt_agent.py` (MIT): one token
//! replaces a set of charge flags. The ledger lives in state.db (Rust only).

use crate::envelope::Envelope;
use crate::exit::Exit;
use rusqlite::{params, Connection, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct Stage {
    pub stage: &'static str,
    pub tool: String,
    pub cost_usd: f64,
    pub network: bool,
    pub explain: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub plan_id: String,
    pub cost_usd: f64,
    pub stages: Vec<Stage>,
    pub downloads: Vec<Value>,
}

/// `p_` plus 16 hex (64 bits) of blake3 over the canonical inputs (serde_json maps
/// are sorted). 64 bits, so nobody can grind a cheap-looking script to match an
/// approved plan id.
pub fn plan_id(inputs: &Value) -> String {
    format!(
        "p_{}",
        &blake3::hash(inputs.to_string().as_bytes()).to_hex()[..16]
    )
}

/// `r_` plus 6 hex of blake3 over plan id and idempotency key: the same
/// command with the same key is the same run.
pub fn run_id_for(plan_id: &str, key: &str) -> String {
    format!(
        "r_{}",
        &blake3::hash(format!("{plan_id}\0{key}").as_bytes()).to_hex()[..6]
    )
}

/// Creates the ledger table if needed.
pub fn ledger_init(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS ledger(
            id INTEGER PRIMARY KEY, run TEXT NOT NULL, provider TEXT NOT NULL,
            est_usd REAL NOT NULL, actual_usd REAL, status TEXT NOT NULL, created TEXT NOT NULL);",
    )
}

/// Reserves `est` for `run` if the run's reserved plus committed spend stays
/// within `cap`. `BEGIN IMMEDIATE` takes the write lock first, so two parallel
/// reserves can never both pass the cap. Returns the reservation id.
pub fn reserve(
    c: &mut Connection,
    run: &str,
    provider: &str,
    est: f64,
    cap: f64,
) -> Result<i64, String> {
    ledger_init(c).map_err(|e| e.to_string())?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let spent: f64 = tx
        .query_row("SELECT COALESCE(SUM(COALESCE(actual_usd, est_usd)), 0) FROM ledger WHERE run = ?1 AND status IN ('reserved', 'committed')", [run], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    // A step that costs nothing cannot break a cap: a free row asks with its own cap of 0 after the run's paid rows
    // have spent, and it must still run.
    if est > 1e-9 && spent + est > cap + 1e-9 {
        return Err(format!(
            "spend cap ${cap:.2} reached: ${spent:.2} spent, ${est:.2} more asked"
        ));
    }
    tx.execute("INSERT INTO ledger(run, provider, est_usd, status, created) VALUES (?1, ?2, ?3, 'reserved', ?4)", params![run, provider, est, crate::run::now_utc()])
        .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Settles a reservation with the real cost (or releases it with `None`).
pub fn settle(c: &Connection, id: i64, actual: Option<f64>) -> rusqlite::Result<usize> {
    match actual {
        Some(a) => c.execute(
            "UPDATE ledger SET actual_usd = ?2, status = 'committed' WHERE id = ?1",
            params![id, a],
        ),
        None => c.execute(
            "UPDATE ledger SET status = 'released' WHERE id = ?1",
            params![id],
        ),
    }
}

/// What `run` has committed in the ledger. A job a resume took over from its parent stays billed to the parent.
pub fn spent(run: &str) -> f64 {
    crate::run::open_state(std::path::Path::new("."))
        .ok()
        .and_then(|(c, _)| {
            c.query_row(
                "SELECT COALESCE(SUM(actual_usd), 0) FROM ledger WHERE run = ?1 AND status = 'committed'",
                [run],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or(0.0)
}

/// This process's `--approve`, for a paid step inside a command that has no plan of its own (a paid speech row, G141).
static APPROVE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

pub fn set_approve(a: Option<String>) {
    let _ = APPROVE.set(a);
}

pub fn approved() -> Option<&'static str> {
    APPROVE.get().and_then(|a| a.as_deref())
}

/// This process's `--local-only`, for the steps under a command that send to a service: a speech row from `analyze`,
/// `edit`, `check`, `dub` or `clips`, and the vision service from `judge` (12 Nov 2026: only `make` read the flag, and
/// `analyze --transcript --local-only` sent the sound to a network row).
static LOCAL_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_local_only(on: bool) {
    LOCAL_ONLY.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn local_only() -> bool {
    LOCAL_ONLY.load(std::sync::atomic::Ordering::Relaxed)
}

/// What a step that needs the network says under `--local-only`. Nothing was sent.
pub const LOCAL_ONLY_STOP: &str =
    "--local-only is on, and this step sends to a service on the network. Nothing was sent";

/// A plan over the caller's cap stops here, before anyone is told to approve it. The plan id stays on the envelope.
/// The ledger's reserve is still the guard that holds at run time.
pub fn over_cap(
    command: &str,
    plan_id: &str,
    cost_usd: f64,
    max_spend: Option<f64>,
    cheaper: Vec<String>,
) -> Option<Envelope> {
    // 1e-9: a price times seconds can land a hair above the cap it equals.
    let cap = max_spend.filter(|cap| cost_usd > *cap + 1e-9)?;
    // Rounded up to a cent, so the amount named here passes.
    let need = ((cost_usd - 1e-9) * 100.0).ceil() / 100.0;
    let mut env = Envelope::fail_code(
        command,
        Exit::ApprovalNeeded,
        "SVX_SPEND_CAP",
        format!("this plan costs ${cost_usd:.4}, over your --max-spend of ${cap:.4}. Nothing ran."),
        Some(format!(
            "Raise --max-spend to {need:.2} or more, or pick a cheaper row."
        )),
    );
    env.status = "approval_needed";
    env.plan_id = Some(plan_id.to_string());
    env.metrics.insert("cost_usd".into(), json!(cost_usd));
    env.next = vec![format!(
        "Raise --max-spend to {need:.2} or more, then plan again."
    )];
    if cheaper.is_empty() {
        env.next
            .push("scriptvox providers --ready --free".to_string());
    }
    env.next.extend(cheaper);
    Some(env)
}

/// An approval-needed envelope from a message that ends with `plan_id <id>` (the form `opentts::stt_gate` writes).
pub fn approval_fail(command: &str, msg: String) -> crate::envelope::Envelope {
    let id = msg
        .rsplit_once("plan_id ")
        .map(|(_, id)| id.trim().to_string());
    let fix = id.as_ref().map_or(
        "Run the same command with --approve <plan_id>.".to_string(),
        |id| format!("Run the same command with --approve {id}"),
    );
    let mut env = crate::envelope::Envelope::fail(
        command,
        crate::exit::Exit::ApprovalNeeded,
        msg,
        Some(fix.clone()),
    );
    env.status = "approval_needed";
    env.plan_id = id;
    env.next = vec![fix];
    env
}

/// The approval answer for a plan: None when it may run, else the envelope fields.
pub fn needs_approval(plan_id: &str, cost_usd: f64, approve: Option<&str>) -> Option<Value> {
    if cost_usd <= 0.0 || approve == Some(plan_id) {
        return None;
    }
    let stale = approve.is_some_and(|a| a != plan_id);
    Some(json!({
        "plan_id": plan_id,
        "cost_usd": cost_usd,
        "reason": if stale { "the plan changed since it was approved (a new plan_id)" } else { "this plan costs money" },
    }))
}

/// The core's side of `ledger.reserve` for one run: reserve against the cap,
/// release what a failed call reserved, settle with the reported cost.
pub struct Ledger {
    pub run: String,
    pub cap: f64,
    pending: std::cell::RefCell<Vec<i64>>,
}

/// RPC error code for a refused reserve.
pub const RESERVE_REFUSED: i64 = -32020;

impl Ledger {
    pub fn new(run: &str, cap: f64) -> Self {
        Self {
            run: run.to_string(),
            cap,
            pending: Default::default(),
        }
    }

    /// Handles one worker request.
    pub fn handle(&self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        if method != "ledger.reserve" {
            return Err((-32601, format!("unknown method {method}")));
        }
        let provider = params["provider"].as_str().unwrap_or("unknown");
        let est = params["est_usd"].as_f64().unwrap_or(f64::INFINITY);
        let (mut c, _) = crate::run::open_state(std::path::Path::new("."))
            .map_err(|e| (RESERVE_REFUSED, e.to_string()))?;
        let id = reserve(&mut c, &self.run, provider, est, self.cap)
            .map_err(|e| (RESERVE_REFUSED, e))?;
        self.pending.borrow_mut().push(id);
        Ok(json!({"reservation": id}))
    }

    /// After the tool call. Success commits the reported cost (else the estimate).
    /// A failure releases the reserve only when nothing was charged (`not_charged`:
    /// the reserve was refused, or the provider refused the request); any other
    /// failure may have been charged, so the estimate stays committed.
    pub fn finish(&self, outcome: Result<Option<f64>, bool>) {
        let Ok((c, _)) = crate::run::open_state(std::path::Path::new(".")) else {
            return;
        };
        for id in self.pending.borrow_mut().drain(..) {
            let est: Option<f64> = c
                .query_row("SELECT est_usd FROM ledger WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .ok();
            let actual = match outcome {
                Ok(cost) => cost.filter(|c| *c > 0.0).or(est),
                Err(true) => None,
                Err(false) => est,
            };
            let _ = settle(&c, id, actual);
        }
    }
}
