//! The one exit-code table (PLAN-R1 §2.3). Doctor, the skill, the radar and the
//! definition of done all read it from here. A snapshot test locks it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok,
    Error,
    Usage,
    GateFailed,
    ApprovalNeeded,
    ProviderRetryable,
    UnconfirmedCharge,
    WatchdogKill,
    Cancelled,
    StillRunning,
    NeedsInput,
    Internal,
}

impl Exit {
    pub const ALL: [Exit; 12] = [
        Exit::Ok,
        Exit::Error,
        Exit::Usage,
        Exit::GateFailed,
        Exit::ApprovalNeeded,
        Exit::ProviderRetryable,
        Exit::UnconfirmedCharge,
        Exit::WatchdogKill,
        Exit::Cancelled,
        Exit::StillRunning,
        Exit::NeedsInput,
        Exit::Internal,
    ];

    pub fn code(self) -> u8 {
        match self {
            Exit::Ok => 0,
            Exit::Error => 1,
            Exit::Usage => 2,
            Exit::GateFailed => 3,
            Exit::ApprovalNeeded => 4,
            Exit::ProviderRetryable => 5,
            Exit::UnconfirmedCharge => 6,
            Exit::WatchdogKill => 7,
            Exit::Cancelled => 8,
            Exit::StillRunning => 9,
            Exit::NeedsInput => 10,
            Exit::Internal => 70,
        }
    }

    /// Stable machine name, also the `SVX_` error-code suffix.
    pub fn name(self) -> &'static str {
        match self {
            Exit::Ok => "OK",
            Exit::Error => "ERROR",
            Exit::Usage => "USAGE",
            Exit::GateFailed => "GATE_FAILED",
            Exit::ApprovalNeeded => "APPROVAL_NEEDED",
            Exit::ProviderRetryable => "PROVIDER_RETRYABLE",
            Exit::UnconfirmedCharge => "UNCONFIRMED_CHARGE",
            Exit::WatchdogKill => "WATCHDOG_KILL",
            Exit::Cancelled => "CANCELLED",
            Exit::StillRunning => "STILL_RUNNING",
            Exit::NeedsInput => "NEEDS_INPUT",
            Exit::Internal => "INTERNAL",
        }
    }

    pub fn meaning(self) -> &'static str {
        match self {
            Exit::Ok => "ok, also a detached start",
            Exit::Error => "error, fixable",
            Exit::Usage => "usage error",
            Exit::GateFailed => "a quality gate failed",
            Exit::ApprovalNeeded => "approval needed for this plan",
            Exit::ProviderRetryable => "provider error, retry is safe",
            Exit::UnconfirmedCharge => "unconfirmed charge, do NOT retry",
            Exit::WatchdogKill => "watchdog killed a stalled step",
            Exit::Cancelled => "cancelled",
            Exit::StillRunning => "still running (wait timeout only)",
            Exit::NeedsInput => "needs input from a human or the agent",
            Exit::Internal => "internal bug",
        }
    }

    pub fn error_code(self) -> String {
        format!("SVX_{}", self.name())
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(e: Exit) -> Self {
        std::process::ExitCode::from(e.code())
    }
}
